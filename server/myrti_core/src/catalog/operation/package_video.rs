use std::ffi::OsString;

use eyre::{eyre, Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};

use crate::{
    catalog::{
        encoding_target::{audio_codec_name, codec_name, VideoEncodingTarget},
        storage_key,
    },
    config,
    core::storage::{Storage, StorageProvider},
    interact,
    model::{
        repository::{self, db::DbPool},
        AssetId, AudioRepresentation, AudioRepresentationId, CreateAudioRepresentation,
        CreateVideoRepresentation, Size, VideoRepresentation,
    },
    processing::{
        self,
        commands::FFmpeg,
        process_control::ProcessControl,
        video::{
            ffmpeg::{FFmpegLocalOutputTrait, FFmpegTrait},
            ffprobe_get_streams,
            gpac::{CreateGHIOptions, DasherOptions},
            mp4_rotate::copy_mp4_rotation_metadata,
            mpd::{self, BaseURL},
            transcode::{ffmpeg_audio_flags, ffmpeg_video_flags},
        },
    },
    util::OptionPathExt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVideo {
    pub asset_id: AssetId,
    pub repr_name: String,
    pub output_key: String,
    pub task: PackageVideoTask,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageVideoTask {
    TranscodeVideo(VideoEncodingTarget),
    TranscodeAudio(AudioEncodingTarget),
    CreateGHIIndex {
        include_video: bool,
        include_audio: bool,
    },
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

// #[instrument(skip(pool, storage, process_control_recv), level = "debug")]
pub async fn do_package_video(
    pool: &DbPool,
    storage: &Storage,
    package_video: PackageVideo,
    bin_paths: Option<&config::BinPaths>,
    mut process_control_recv: mpsc::Receiver<ProcessControl>,
) -> Result<()> {
    let asset_id = package_video.asset_id;
    let conn = pool.get().await?;
    let asset_path = interact!(conn, move |conn| {
        repository::asset::get_asset_path_on_disk(conn, asset_id)
    })
    .await??;

    let ffmpeg_path = bin_paths.and_then(|bp| bp.ffmpeg.as_opt_path());
    let ffprobe_path = bin_paths.and_then(|bp| bp.ffprobe.as_opt_path());
    let gpac_path = bin_paths.and_then(|bp| bp.gpac.as_opt_path());

    match &package_video.task {
        PackageVideoTask::CreateGHIIndex {
            include_video,
            include_audio,
        } => {
            // let max_iframe_interval = interact!(conn, move |conn| repository::asset::set_asset_max_iframe_interval)
            let segment_duration = 2; // TODO
            let out_dir = match storage {
                Storage::LocalFileStorage(local_file_storage) => {
                    local_file_storage.root.join(&package_video.output_key)
                }
            };
            processing::video::gpac::create_ghi_and_manifest(
                &asset_path.path_on_disk(),
                &CreateGHIOptions {
                    segment_duration,
                    video_rep_id: include_video.then(|| String::from("original_video")),
                    audio_rep_id: include_audio.then(|| String::from("original_audio")),
                    mpd_base_url: None,
                    ghi_out_path: out_dir.join("index.ghi"),
                    mpd_out_path: out_dir.join("stream.mpd"),
                },
                gpac_path,
                &mut process_control_recv,
            )
            .await?;
            let has_ghi = match (include_video, include_audio) {
                (true, true) => 3,
                (false, true) => 2,
                (true, false) => 1,
                (false, false) => 0,
            };
            interact!(conn, move |conn| {
                repository::asset::set_asset_has_ghi_index(conn, asset_id, has_ghi)
            })
            .await??;
        }
        PackageVideoTask::TranscodeAudio(transcode) => {
            let repr_name = package_video.repr_name.clone();
            let codec_name = audio_codec_name(transcode);

            let codec_name2 = codec_name.clone();
            let repr_id = interact!(conn, move |conn| {
                repository::representation::insert_audio_representation(
                    conn,
                    &CreateAudioRepresentation {
                        asset_id,
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

            FFmpeg::new(
                Vec::default(),
                ffmpeg_audio_flags(transcode)
                    .into_iter()
                    .map(OsString::from)
                    .collect(),
            )
            .run_with_local_output(
                asset_path.path_on_disk().as_str(),
                &utf8_path,
                ffmpeg_path,
                &mut process_control_recv,
            )
            .await
            .context("error transcoding audio with ffmpeg")?;

            let out_dir = match storage {
                Storage::LocalFileStorage(local_file_storage) => local_file_storage.root.join(
                    storage_key::dash_file(asset_id, format_args!("{}", &repr_file_stem)),
                ),
            };
            let mpd_name = "stream.mpd";
            tokio::fs::create_dir(&out_dir).await?;
            let _dash_result = processing::video::gpac::run_dasher(
                &utf8_path,
                &out_dir,
                DasherOptions {
                    mpd_name,
                    base_url: None,
                },
                gpac_path,
                &mut process_control_recv,
            )
            .await?;
            interact!(conn, move |conn| {
                repository::representation::finalize_audio_representation(conn, repr_id)
            })
            .await??;
        }
        PackageVideoTask::TranscodeVideo(transcode) => {
            let repr_name = package_video.repr_name.clone();
            let codec_name = codec_name(&transcode.codec);
            let codec_name2 = codec_name.to_owned();
            let repr_id = interact!(conn, move |conn| {
                repository::representation::insert_video_representation(
                    conn,
                    &CreateVideoRepresentation {
                        asset_id,
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

            let pre_input_flags = vec![OsString::from("-noautorotate")];
            FFmpeg::new(
                pre_input_flags,
                ffmpeg_video_flags(transcode)
                    .into_iter()
                    .chain(std::iter::once("-an".to_owned()))
                    .map(OsString::from)
                    .collect(),
            )
            .run_with_local_output(
                asset_path.path_on_disk().as_str(),
                &utf8_path,
                ffmpeg_path,
                &mut process_control_recv,
            )
            .await
            .context("error transcoding audio with ffmpeg")?;

            let out_dir = match storage {
                Storage::LocalFileStorage(local_file_storage) => local_file_storage.root.join(
                    storage_key::dash_file(asset_id, format_args!("{}", &repr_file_stem)),
                ),
            };

            let mpd_name = "stream.mpd";

            tokio::fs::create_dir(&out_dir).await?;
            let dash_result = processing::video::gpac::run_dasher(
                &utf8_path,
                &out_dir,
                DasherOptions {
                    mpd_name,
                    base_url: None,
                },
                gpac_path,
                &mut process_control_recv,
            )
            .await?;

            copy_mp4_rotation_metadata(
                asset_path.path_on_disk().as_std_path(),
                dash_result.mp4_path.as_std_path(),
            )
            .await
            .context(
                "error copying mp4 rotation metadata from original asset to new representation",
            )?;

            let (_, streams) = ffprobe_get_streams(&dash_result.mp4_path, ffprobe_path).await?;
            let repr_name = package_video.repr_name.clone();
            interact!(conn, move |conn| {
                repository::representation::finalize_video_representation(
                    conn,
                    &VideoRepresentation {
                        id: repr_id,
                        asset_id,
                        name: repr_name.to_owned(),
                        codec_name: codec_name.to_owned(),
                        width: streams.video.width,
                        height: streams.video.height,
                        bitrate: streams.video.bitrate,
                    },
                )
            })
            .await??;
        }
    }

    // merge MPD manifests
    let existing_video_reprs = interact!(conn, move |conn| {
        repository::representation::get_video_representations(conn, asset_id)
    })
    .await??;
    let existing_audio_reprs = interact!(conn, move |conn| {
        repository::representation::get_audio_representations(conn, asset_id)
    })
    .await??;

    let mut merged_manifest = None;
    let mut adaptation_sets = vec![];

    let repr_info = existing_video_reprs
        .into_iter()
        .map(|repr| (repr.id.0, repr.name))
        .chain(
            existing_audio_reprs
                .into_iter()
                .map(|repr| (repr.id.0, repr.name)),
        );

    for (repr_id, repr_name) in repr_info {
        let mpd_key = storage_key::dash_file(
            asset_id,
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

        let mut adaptations = manifest.periods[0].adaptations.clone();
        let prepend_base = format_args!("{}-{}", repr_id, repr_name);
        for adap in &mut adaptations {
            for rep in &mut adap.representations {
                if rep.BaseURL.is_empty() {
                    rep.BaseURL.push(BaseURL {
                        base: format!("{prepend_base}"),
                        ..Default::default()
                    })
                } else {
                    for base in &mut rep.BaseURL {
                        base.base = format!("{prepend_base}/{}", base.base);
                    }
                }
            }
        }
        adaptation_sets.extend(adaptations);
        if merged_manifest.is_none() {
            merged_manifest = Some(manifest)
        }
    }
    let has_ghi = interact!(conn, move |conn| {
        repository::asset::get_asset_has_ghi_index(conn, asset_id)
    })
    .await??;
    if has_ghi.is_some_and(|s| s != 0) {
        let mpd_key = storage_key::dash_file(asset_id, format_args!("original/stream.mpd"));
        let mut mpd_str = String::new();
        storage
            .open_read_stream(&mpd_key)
            .await?
            .read_to_string(&mut mpd_str)
            .await?;
        let manifest = mpd::parse(&mpd_str)?;

        let mut adaptations = manifest.periods[0].adaptations.clone();
        let prepend_base = "original";
        for adap in &mut adaptations {
            adap.representations
                .retain(|rep| rep.id.as_deref() != Some("ignored"));
            for rep in &mut adap.representations {
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
        }
        adaptation_sets.extend(
            adaptations
                .into_iter()
                .filter(|adap| !adap.representations.is_empty()),
        );
        if merged_manifest.is_none() {
            merged_manifest = Some(manifest);
        }
    }

    // TODO: set full profile
    let mut merged_manifest = merged_manifest.unwrap();
    merged_manifest.periods[0].adaptations = adaptation_sets;
    storage
        .open_write_stream(&storage_key::dash_file(
            asset_id,
            format_args!("stream.mpd"),
        ))
        .await?
        .write_all(merged_manifest.to_string().as_bytes())
        .await?;

    Ok(())
}
