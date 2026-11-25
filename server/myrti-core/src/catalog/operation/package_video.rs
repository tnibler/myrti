use std::ffi::OsString;

use eyre::{Context, Result, eyre};
use itertools::Itertools;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};

use myrti_data::db::{DbPool, PooledDbConn};
use myrti_data::model::{
    CreateAudioRepresentation, CreateVideoRepresentation, FileId, IsOriginalStreamable, Size,
    VideoRepresentation,
};
use myrti_data::{interact, repository};

use crate::{
    catalog::{
        encoding_target::{VideoEncodingTarget, audio_codec_name, codec_name},
        storage_key,
    },
    config,
    core::storage::Storage,
    processing::{
        self,
        process_control::ProcessControl,
        video::{
            dash_index,
            ffmpeg::run_ffmpeg,
            ffprobe_get_streams,
            mp4_rotate::copy_mp4_rotation_metadata,
            mpd::{self, BaseURL},
            transcode::{ffmpeg_audio_flags, ffmpeg_video_flags},
        },
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct PackageVideo {
    pub file_id: FileId,
    pub repr_name: String,
    pub output_key: String,
    pub task: PackageVideoTask,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PackageVideoTask {
    TranscodeVideo(VideoEncodingTarget),
    TranscodeAudio(AudioEncodingTarget),
    CreateGHIIndex {
        include_video: bool,
        include_audio: bool,
        segment_duration: f32,
    },
    DashSegment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTranscodeResult {
    pub target: VideoEncodingTarget,
    pub final_size: Size,
    pub bitrate: i64,
    pub out_file_key: String,
    pub out_media_info_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioEncodingTarget {
    AAC,
    OPUS,
    FLAC,
    MP3,
}

#[tracing::instrument(skip_all, fields(?package_video), level = "debug")]
pub async fn do_package_video(
    pool: &DbPool,
    storage: &Storage,
    package_video: PackageVideo,
    bin_paths: &config::BinPaths,
    mut process_control_recv: mpsc::Receiver<ProcessControl>,
) -> Result<()> {
    let file_id = package_video.file_id;
    let mut conn = pool.get().await?;
    let (video, file) = interact!(conn, move |conn| {
        repository::asset::get_video_file(conn, file_id)
    })
    .await??;
    let file_path = interact!(conn, move |conn| {
        repository::asset::get_asset_path_on_disk(conn, video.file_id)
    })
    .await??;

    let ffmpeg_path = bin_paths.ffmpeg_path();
    let ffprobe_path = bin_paths.ffprobe_path();

    let asset_dash_dir = storage.local_path(&storage_key::dash_file(file_id, format_args!("")));
    tokio::fs::create_dir_all(&asset_dash_dir)
        .await
        .wrap_err_with(|| format!("error creating directory {}", asset_dash_dir))?;

    match &package_video.task {
        PackageVideoTask::DashSegment => {
            let orig_dash_dir =
                storage.local_path(&storage_key::dash_file(file_id, format_args!("original")));
            tracing::trace!(path=?orig_dash_dir, ?file_id, "deleting DASH files");
            match tokio::fs::remove_dir_all(&orig_dash_dir).await {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
            .wrap_err("error deleting DASH data directory")?;
            tokio::fs::create_dir(&orig_dash_dir).await?;

            let in_path = interact!(conn, move |conn| {
                repository::asset::get_asset_path_on_disk(conn, file_id)
            })
            .await??
            .path_on_disk();

            let mpd_name = "stream.mpd";
            let _mp4_path = processing::video::ffmpeg::ffmpeg_package_dash(
                &in_path,
                &orig_dash_dir,
                mpd_name,
                2.0, // TODO
                ffmpeg_path,
                &mut process_control_recv,
            )
            .await?;

            interact!(conn, move |conn| {
                repository::asset::set_asset_original_streamable_done(conn, file_id, true)
            })
            .await??;
        }
        PackageVideoTask::CreateGHIIndex {
            include_video,
            include_audio,
            segment_duration,
        } => {
            let out_dir = storage
                .local_path(&package_video.output_key)
                .into_std_path_buf();
            let file_path = file_path.path_on_disk().into_std_path_buf();
            let segment_duration = *segment_duration;

            match tokio::fs::create_dir(&out_dir).await {
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                result => result,
            }
            .wrap_err_with(|| format!("error creating directory {}", out_dir.display()))?;
            dash_index::index_file(
                &file_path,
                &out_dir,
                "original",
                "stream.mpd",
                segment_duration,
            )
            .await?;
        }
        PackageVideoTask::TranscodeAudio(transcode) => {
            let repr_name = package_video.repr_name.clone();
            let codec_name = audio_codec_name(transcode);

            let codec_name2 = codec_name.clone();
            let repr_id = interact!(conn, move |conn| {
                repository::representation::insert_audio_representation(
                    conn,
                    &CreateAudioRepresentation {
                        file_id,
                        codec_name: codec_name2,
                        name: repr_name,
                    },
                )
            })
            .await??;

            let repr_file_stem = format!("{}-{}", repr_id.0, package_video.repr_name);
            let ffmpeg_out_dir = tempfile::tempdir()?;
            let ffmpeg_out_path = ffmpeg_out_dir
                .path()
                .join(format!("{}.mp4", repr_file_stem));
            let utf8_path: camino::Utf8PathBuf = ffmpeg_out_path
                .to_path_buf()
                .try_into()
                .expect("temp files should have utf8 paths");

            let pre_input_flags = [];
            let audio_flags = ffmpeg_audio_flags(transcode)
                .into_iter()
                .chain(["-vn".to_owned()])
                .map(OsString::from)
                .collect_vec();
            run_ffmpeg(
                file_path.path_on_disk().as_str(),
                &utf8_path,
                &pre_input_flags,
                &audio_flags,
                ffmpeg_path,
                &mut process_control_recv,
            )
            .await
            .context("error transcoding audio with ffmpeg")?;

            let out_dir = asset_dash_dir.join(&repr_file_stem);
            tokio::fs::create_dir(&out_dir)
                .await
                .wrap_err_with(|| format!("error creating directory {}", out_dir))?;
            let mpd_name = "stream.mpd";
            let _mp4_path = processing::video::ffmpeg::ffmpeg_package_dash(
                &utf8_path,
                &out_dir,
                mpd_name,
                5.0, // TODO
                ffmpeg_path,
                &mut process_control_recv,
            )
            .await?;
            interact!(conn, move |conn| {
                repository::representation::finalize_audio_representation(conn, repr_id)
            })
            .await??;
        }
        PackageVideoTask::TranscodeVideo(transcode) => {
            tracing::info!(?transcode, "video transcode");
            let repr_name = package_video.repr_name.clone();
            let codec_name = codec_name(&transcode.codec);
            let codec_name2 = codec_name.to_owned();
            let repr_id = interact!(conn, move |conn| {
                repository::representation::insert_video_representation(
                    conn,
                    &CreateVideoRepresentation {
                        file_id,
                        name: repr_name,
                        codec_name: codec_name2,
                    },
                )
            })
            .await??;

            let repr_file_stem = format!("{}-{}", repr_id.0, package_video.repr_name);
            let ffmpeg_out_dir = tempfile::tempdir()?;
            let ffmpeg_out_path = ffmpeg_out_dir
                .path()
                .join(format!("{}.mp4", repr_file_stem));
            let utf8_path: camino::Utf8PathBuf = ffmpeg_out_path
                .to_path_buf()
                .try_into()
                .expect("temp files should have utf8 paths");

            let no_autorotate = file.file_type == "mp4";
            let pre_input_flags = if no_autorotate {
                vec![OsString::from("-noautorotate")]
            } else {
                vec![]
            };
            let video_flags = ffmpeg_video_flags(transcode)
                .into_iter()
                .chain(std::iter::once("-an".to_owned()))
                .map(OsString::from)
                .collect_vec();
            run_ffmpeg(
                file_path.path_on_disk().as_str(),
                &utf8_path,
                &pre_input_flags,
                &video_flags,
                ffmpeg_path,
                &mut process_control_recv,
            )
            .await
            .context("error transcoding audio with ffmpeg")?;

            let out_dir = asset_dash_dir.join(&repr_file_stem);
            tokio::fs::create_dir(&out_dir)
                .await
                .wrap_err_with(|| format!("error creating directory {}", out_dir))?;
            let mpd_name = "stream.mpd";
            tracing::debug!("running ffmpeg dash segmenter");
            let mp4_path = processing::video::ffmpeg::ffmpeg_package_dash(
                &utf8_path,
                &out_dir,
                mpd_name,
                2.0, // TODO
                ffmpeg_path,
                &mut process_control_recv,
            )
            .await?;

            if no_autorotate {
                copy_mp4_rotation_metadata(
                    file_path.path_on_disk().as_std_path(),
                    mp4_path.as_std_path(),
                )
                .await
                .context(
                    "error copying mp4 rotation metadata from original asset to new representation",
                )?;
            }

            let (_, streams) = ffprobe_get_streams(&mp4_path, ffprobe_path).await?;
            let bitrate = streams.video.bitrate.ok_or_else(|| {
                eyre!("transcoded mp4 has no bitrate in stream info: {}", mp4_path)
            })?;
            let repr_name = package_video.repr_name.clone();
            interact!(conn, move |conn| {
                repository::representation::finalize_video_representation(
                    conn,
                    &VideoRepresentation {
                        id: repr_id,
                        file_id,
                        name: repr_name.to_owned(),
                        codec_name: codec_name.to_owned(),
                        width: streams.video.width,
                        height: streams.video.height,
                        bitrate,
                    },
                )
            })
            .await??;
        }
    }

    create_merged_manifest(&mut conn, storage, file_id)
        .await
        .wrap_err("error creating merged MPD manifes")?;

    if let PackageVideoTask::CreateGHIIndex { .. } | PackageVideoTask::DashSegment =
        package_video.task
    {
        interact!(conn, move |conn| {
            repository::asset::set_asset_original_streamable_done(conn, file_id, true)
        })
        .await??;
    }

    Ok(())
}

#[tracing::instrument(skip_all, fields(file_id), level = "trace")]
async fn create_merged_manifest(
    conn: &mut PooledDbConn,
    storage: &Storage,
    file_id: FileId,
) -> Result<()> {
    // merge MPD manifests
    let existing_video_reprs = interact!(conn, move |conn| {
        repository::representation::get_video_representations(conn, file_id)
    })
    .await??;
    let existing_audio_reprs = interact!(conn, move |conn| {
        repository::representation::get_audio_representations(conn, file_id)
    })
    .await??;

    let mut merged_manifest = None;
    let mut adaptation_set_video = None;
    let mut adaptation_set_audio = None;
    let mut adaptation_set_orig_video = None;
    let mut adaptation_set_orig_audio = None;
    let mut mpd_reprs_video = vec![];
    let mut mpd_reprs_audio = vec![];

    let repr_info = existing_video_reprs
        .into_iter()
        .map(|repr| (repr.id.0, repr.name))
        .chain(
            existing_audio_reprs
                .into_iter()
                .map(|repr| (repr.id.0, repr.name)),
        );

    let mut max_segment_duration: Option<std::time::Duration> = None;
    for (repr_id, repr_name) in repr_info {
        let mpd_key = storage_key::dash_file(
            file_id,
            format_args!("{}-{}/stream.mpd", repr_id, repr_name),
        );
        let mut mpd_str = String::new();
        storage
            .open_read_stream(&mpd_key)
            .await?
            .read_to_string(&mut mpd_str)
            .await?;
        let manifest = mpd::parse(&mpd_str)?;
        debug_assert_eq!(manifest.periods.len(), 1);
        max_segment_duration =
            max_segment_duration.and_then(|md| manifest.maxSegmentDuration.map(|md2| md.max(md2)));

        let prepend_base = format_args!("{}-{}", repr_id, repr_name);
        for adap in &manifest.periods[0].adaptations {
            let is_video = adap.contentType.as_deref() == Some("video")
                || adap.mimeType.as_deref() == Some("video/mp4");
            let is_audio = adap.contentType.as_deref() == Some("audio")
                || adap.mimeType.as_deref() == Some("audio/mp4");
            if is_video && adaptation_set_video.is_none() {
                adaptation_set_video = Some(adap.clone());
            }
            if is_audio && adaptation_set_audio.is_none() {
                adaptation_set_audio = Some(adap.clone());
            }
            for mut rep in adap.representations.clone() {
                if rep.BaseURL.is_empty() {
                    rep.BaseURL.push(BaseURL {
                        base: format!("{prepend_base}/"),
                        ..Default::default()
                    })
                } else {
                    for base in &mut rep.BaseURL {
                        base.base = format!("{prepend_base}/{}", base.base);
                    }
                }
                if is_video {
                    mpd_reprs_video.push(rep);
                } else if is_audio {
                    mpd_reprs_audio.push(rep);
                }
            }
        }
        if merged_manifest.is_none() {
            merged_manifest = Some(manifest)
        }
    }
    let has_ghi = interact!(conn, move |conn| {
        repository::asset::get_asset_has_ghi_index(conn, file_id)
    })
    .await??;
    if has_ghi.is_some_and(|s| s != IsOriginalStreamable::None) {
        let mpd_key = storage_key::dash_file(file_id, format_args!("original/stream.mpd"));
        let mut mpd_str = String::new();
        storage
            .open_read_stream(&mpd_key)
            .await?
            .read_to_string(&mut mpd_str)
            .await?;
        let manifest = mpd::parse(&mpd_str)?;
        max_segment_duration =
            max_segment_duration.and_then(|md| manifest.maxSegmentDuration.map(|md2| md.max(md2)));

        let prepend_base = "original";
        for mut adap in manifest.periods[0].adaptations.clone() {
            let is_video = adap.contentType.as_deref() == Some("video")
                || adap.mimeType.as_deref() == Some("video/mp4");
            let is_audio = adap.contentType.as_deref() == Some("audio")
                || adap.mimeType.as_deref() == Some("audio/mp4");
            for rep in &mut adap.representations {
                if let Some(codecs) = rep.codecs.as_mut() {
                    codecs.make_ascii_lowercase();
                    // 0x00 is an invalid level but can be found in some old files and will not play
                    // in browsers.
                    // Set it to baseline @ 3.1 as a safe bet.
                    *codecs = codecs.replace("avc1.42e000", "avc1.42e01f");
                }
                if rep.BaseURL.is_empty() {
                    rep.BaseURL.push(BaseURL {
                        base: format!("{prepend_base}/"),
                        ..Default::default()
                    })
                } else {
                    for base in &mut rep.BaseURL {
                        base.base = format!("{prepend_base}/{}", base.base);
                    }
                }
            }
            if is_video {
                adaptation_set_orig_video = Some(adap);
            } else if is_audio {
                adaptation_set_orig_audio = Some(adap);
            }
        }
        if merged_manifest.is_none() {
            merged_manifest = Some(manifest);
        }
    }

    debug_assert_eq!(adaptation_set_audio.is_none(), mpd_reprs_audio.is_empty());
    debug_assert_eq!(adaptation_set_video.is_none(), mpd_reprs_video.is_empty());
    for (idx, rep) in mpd_reprs_video.iter_mut().enumerate() {
        rep.id = Some(format!("{idx}"));
    }
    for (idx, rep) in mpd_reprs_audio.iter_mut().enumerate() {
        rep.id = Some(format!("{idx}"));
    }
    if let Some(adap) = &mut adaptation_set_video {
        adap.representations = mpd_reprs_video;
    }
    if let Some(adap) = &mut adaptation_set_audio {
        adap.representations = mpd_reprs_audio;
    }

    let mut merged_manifest = merged_manifest.expect("at least one manifest was just created");
    merged_manifest.maxSegmentDuration = max_segment_duration;
    merged_manifest.periods[0].adaptations = adaptation_set_video
        .into_iter()
        .chain(adaptation_set_audio)
        .chain(adaptation_set_orig_video)
        .chain(adaptation_set_orig_audio)
        .collect();

    let mpd_path = storage.local_path(&storage_key::dash_file(file_id, format_args!("stream.mpd")));
    tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&mpd_path)
        .await
        .wrap_err_with(|| format!("error opening file {mpd_path} for writing"))?
        .write_all(merged_manifest.to_string().as_bytes())
        .await
        .wrap_err_with(|| format!("error writing mpd manifest at {mpd_path}"))?;

    Ok(())
}
