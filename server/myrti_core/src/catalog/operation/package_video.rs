use std::ffi::OsString;

use diesel::Connection;
use eyre::{eyre, Context, Result};
use tokio::sync::mpsc;
use tracing::{error, instrument};

use crate::{
    catalog::{
        encoding_target::{audio_codec_name, codec_name, VideoEncodingTarget},
        storage_key,
    },
    config,
    core::storage::Storage,
    interact,
    model::{
        repository::{
            self,
            db::{DbPool, PooledDbConn},
        },
        AssetId, AudioRepresentation, AudioRepresentationId, Size, Video, VideoAsset,
        VideoRepresentation, VideoRepresentationId,
    },
    processing::{
        self,
        commands::{FFmpeg, FFmpegIntoShaka, MpdGenerator, ShakaIntoFFmpeg, ShakaPackager},
        process_control::ProcessControl,
        video::{
            ffmpeg::{FFmpegLocalOutputTrait, FFmpegTrait},
            ffmpeg_into_shaka::{FFmpegIntoShakaFFmpegTrait, FFmpegIntoShakaTrait},
            ffprobe_get_streams,
            gpac::CreateGHIOptions,
            mpd_generator::MpdGeneratorTrait,
            shaka::{RepresentationType, ShakaPackagerTrait},
            shaka_into_ffmpeg::ShakaIntoFFmpegTrait,
            transcode::{ffmpeg_video_flags, ProduceAudio, ProduceVideo},
            video_rotation::FFProbeRotationTrait,
            FFProbe,
        },
    },
    util::OptionPathExt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateAudioRepr {
    Transcode(AudioTranscode),
    PackageOriginalFile { output_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatedAudioRepr {
    Transcode(AudioTranscodeResult),
    PackagedOriginalFile {
        out_file_key: String,
        out_media_info_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateVideoRepr {
    Transcode(VideoTranscode),
    PackageOriginalFile { output_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateGHIIndex {
    pub include_video: bool,
    pub include_audio: bool,
    pub out_file_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// all paths are relative to the dash resource directory
pub struct PackageVideo {
    pub asset_id: AssetId,
    pub create_video_repr: CreateVideoRepr,
    pub create_audio_repr: Option<CreateAudioRepr>,
    pub create_ghi: Option<CreateGHIIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatedVideoRepr {
    PackagedOriginalFile {
        out_file_key: String,
        out_media_info_key: String,
    },
    Transcode(VideoTranscodeResult),
}

// Some things like the resulting size and bitrate of
// a video we don't actually know until ffmpeg is done.
// That information needs to be known to apply the operation
// to the database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedPackageVideo {
    pub asset_id: AssetId,
    pub created_video_repr: CreatedVideoRepr,
    pub created_audio_repr: Option<CreatedAudioRepr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTranscode {
    pub target: VideoEncodingTarget,
    pub output_key: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTranscode {
    pub target: AudioEncodingTarget,
    pub output_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTranscodeResult {
    pub target: AudioEncodingTarget,
    pub out_file_key: String,
    pub out_media_info_key: String,
}

#[instrument(skip(conn), level = "debug")]
pub async fn apply_package_video(conn: &mut PooledDbConn, op: CompletedPackageVideo) -> Result<()> {
    let asset: VideoAsset = interact!(conn, move |conn| {
        repository::asset::get_asset(conn, op.asset_id)?.try_into()
    })
    .await??;
    let asset = VideoAsset {
        video: Video {
            has_dash: true,
            ..asset.video
        },
        ..asset
    };
    interact!(conn, move |conn| conn.transaction(|conn| {
        repository::asset::set_asset_has_dash(conn, op.asset_id, true)?;
        match &op.created_audio_repr {
            Some(CreatedAudioRepr::Transcode(audio_transcode)) => {
                let audio_representation = AudioRepresentation {
                    id: AudioRepresentationId(0),
                    asset_id: op.asset_id,
                    codec_name: audio_codec_name(&audio_transcode.target),
                    file_key: audio_transcode.out_file_key.clone(),
                    media_info_key: audio_transcode.out_media_info_key.clone(),
                };
                let _audio_representation_id =
                    repository::representation::insert_audio_representation(
                        conn,
                        &audio_representation,
                    )?;
            }
            Some(CreatedAudioRepr::PackagedOriginalFile {
                out_file_key,
                out_media_info_key,
            }) => {
                let audio_representation = AudioRepresentation {
                    id: AudioRepresentationId(0),
                    asset_id: op.asset_id,
                    codec_name: asset.video.audio_codec_name.unwrap(), // TODO
                    file_key: out_file_key.clone(),
                    media_info_key: out_media_info_key.clone(),
                };
                let _audio_representation_id =
                    repository::representation::insert_audio_representation(
                        conn,
                        &audio_representation,
                    )?;
            }
            None => {}
        }
        let created_video_repr = match &op.created_video_repr {
            CreatedVideoRepr::PackagedOriginalFile {
                out_file_key,
                out_media_info_key,
            } => Some(VideoRepresentation {
                id: VideoRepresentationId(0),
                asset_id: asset.base.id,
                codec_name: asset.video.video_codec_name,
                width: asset.base.size.width,
                height: asset.base.size.height,
                bitrate: asset.video.video_bitrate,
                file_key: out_file_key.clone(),
                media_info_key: out_media_info_key.clone(),
            }),
            CreatedVideoRepr::Transcode(transcode) => Some(VideoRepresentation {
                id: VideoRepresentationId(0),
                asset_id: asset.base.id,
                codec_name: codec_name(&transcode.target.codec).to_owned(),
                width: transcode.final_size.width,
                height: transcode.final_size.height,
                bitrate: transcode.bitrate,
                file_key: transcode.out_file_key.clone(),
                media_info_key: transcode.out_media_info_key.clone(),
            }),
        };
        if let Some(video_repr) = created_video_repr {
            let _video_repr_id =
                repository::representation::insert_video_representation(conn, &video_repr)?;
        }
        Ok(())
    }))
    .await?
}

#[instrument(skip(pool, storage, process_control_recv), level = "debug")]
pub async fn perform_side_effects_package_video(
    pool: &DbPool,
    storage: &Storage,
    package_video: &PackageVideo,
    bin_paths: Option<&config::BinPaths>,
    mut process_control_recv: mpsc::Receiver<ProcessControl>,
) -> Result<CompletedPackageVideo> {
    let asset_id = package_video.asset_id;
    let conn = pool.get().await?;
    let asset_path = interact!(conn, move |conn| {
        repository::asset::get_asset_path_on_disk(conn, asset_id)
    })
    .await??;

    let ffmpeg_path = bin_paths.and_then(|bp| bp.ffmpeg.as_opt_path());
    let ffprobe_path = bin_paths.and_then(|bp| bp.ffprobe.as_opt_path());
    let shaka_packager_path = bin_paths.and_then(|bp| bp.shaka_packager.as_opt_path());
    let mpd_generator_path = bin_paths.and_then(|bp| bp.mpd_generator.as_opt_path());
    let gpac_path = bin_paths.and_then(|bp| bp.mpd_generator.as_opt_path());

    let ffmpeg_video_op: Option<ProduceVideo> = match package_video.create_video_repr.clone() {
        CreateVideoRepr::Transcode(video_transcode) => {
            Some(ProduceVideo::Transcode(video_transcode.target))
        }
        _ => None,
    };
    let ffmpeg_audio_op: Option<ProduceAudio> = match package_video.create_audio_repr.clone() {
        Some(CreateAudioRepr::Transcode(audio_transcode)) => {
            Some(ProduceAudio::Transcode(audio_transcode.target))
        }
        _ => None,
    };

    let ffmpeg_into_shaka = if ffmpeg_video_op.is_some() || ffmpeg_audio_op.is_some() {
        let ffmpeg_into_shaka = FFmpegIntoShaka::new(
            asset_path.path_on_disk(),
            ffmpeg_video_op.as_ref(),
            ffmpeg_audio_op.as_ref(),
        );
        Some(
            ffmpeg_into_shaka
                .run_ffmpeg(ffmpeg_path, &mut process_control_recv)
                .await?,
        )
    } else {
        None
    };

    if let Some(CreateGHIIndex {
        include_video,
        include_audio,
        out_file_key,
    }) = &package_video.create_ghi
    {
        tracing::info!(?package_video.create_ghi);
        // let max_iframe_interval = interact!(conn, move |conn| repository::asset::set_asset_max_iframe_interval)
        let segment_duration = 1; // TODO
        let out_dir = match storage {
            Storage::LocalFileStorage(local_file_storage) => {
                local_file_storage.root.join(out_file_key)
            }
        };
        processing::video::gpac::create_ghi_and_manifest(
            &asset_path.path_on_disk(),
            &CreateGHIOptions {
                segment_duration,
                video_rep_id: String::from("original_video"),
                audio_rep_id: String::from("original_audio"),
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
        });
    }

    let created_audio_repr: Option<CreatedAudioRepr> = match &package_video.create_audio_repr {
        Some(CreateAudioRepr::PackageOriginalFile { output_key }) => {
            ShakaPackager::run(
                &asset_path.path_on_disk(),
                RepresentationType::Audio,
                output_key,
                storage,
                shaka_packager_path,
                &mut process_control_recv,
            )
            .await
            .wrap_err("could not shaka package audio stream")?;
            let out_media_info_key = format!("{}.media_info", output_key);
            Some(CreatedAudioRepr::PackagedOriginalFile {
                out_file_key: output_key.clone(),
                out_media_info_key,
            })
        }
        Some(CreateAudioRepr::Transcode(transcode)) => {
            debug_assert!(ffmpeg_into_shaka.is_some());
            let ffmpeg_into_shaka = match ffmpeg_into_shaka.as_ref() {
                Some(f) => f,
                None => {
                    error!("BUG: ffmpeg_into_shaka is None when it should not be");
                    return Err(eyre!(
                        "BUG: ffmpeg_into_shaka is None when it should not be"
                    ));
                }
            };
            let shaka_result = ffmpeg_into_shaka
                .run_shaka_packager(
                    RepresentationType::Audio,
                    &transcode.output_key,
                    storage,
                    shaka_packager_path,
                    &mut process_control_recv,
                )
                .await?;
            Some(CreatedAudioRepr::Transcode(AudioTranscodeResult {
                target: transcode.target.clone(),
                out_file_key: transcode.output_key.clone(),
                out_media_info_key: shaka_result.media_info_key,
            }))
        }
        None => None,
    };

    let created_video_repr = match &package_video.create_video_repr {
        CreateVideoRepr::PackageOriginalFile { output_key } => {
            unimplemented!()
        }
        CreateVideoRepr::Transcode(transcode) => {
            debug_assert!(ffmpeg_into_shaka.is_some());
            let repr_name = transcode.output_key.split("/").last().unwrap();

            let ffmpeg_out_dir = tempfile::tempdir()?;
            let ffmpeg_out_path = ffmpeg_out_dir.path().join(format!("{}.mp4", repr_name));
            let utf8_path: camino::Utf8PathBuf = ffmpeg_out_path
                .to_path_buf()
                .try_into()
                .expect("temp files should have utf8 paths");
            let has_video_ghi = interact!(conn, move |conn| {
                repository::asset::get_asset_has_ghi_index(conn, asset_id)
            })
            .await??
            .is_some_and(|i| i == 1 || i == 3);

            let pre_input_flags = if has_video_ghi {
                vec![OsString::from("-noautorotate")]
            } else {
                vec![]
            };
            FFmpeg::new(
                pre_input_flags,
                ffmpeg_video_flags(&ProduceVideo::Transcode(transcode.target.clone()))
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
            .context("Error transcoding with ffmpeg")?;

            let out_dir = match storage {
                Storage::LocalFileStorage(local_file_storage) => {
                    local_file_storage.root.join(&transcode.output_key)
                }
            };

            let mpd_name = "stream.mpd";

            tokio::fs::create_dir(&out_dir).await?;
            let dash_result = processing::video::gpac::run_dasher(
                &utf8_path,
                &out_dir,
                mpd_name,
                gpac_path,
                &mut process_control_recv,
            )
            .await?;
            tracing::debug!(?dash_result);

            if has_video_ghi {
                // TODO: copy rotation from original to ffmpeg output
            }
            todo!()

            // let probe = todo!();
            // TODO: merge mpd manifests
            // CreatedVideoRepr::Transcode(VideoTranscodeResult {
            //     target: transcode.target.clone(),
            //     final_size: Size {
            //         width: probe.width,
            //         height: probe.height,
            //     },
            //     bitrate: probe.bitrate,
            //     out_file_key: transcode.output_key.clone(),
            //     out_media_info_key: todo!(),
            // })
        }
    };

    // mpd_generator needs media_infos as local files
    // We just copy the
    let mut media_info_keys: Vec<String> = Vec::default();
    media_info_keys.push(match &created_video_repr {
        CreatedVideoRepr::Transcode(transcode) => transcode.out_media_info_key.clone(),
        CreatedVideoRepr::PackagedOriginalFile {
            out_file_key: _,
            out_media_info_key,
        } => out_media_info_key.clone(),
    });
    if let Some(audio_repr) = &created_audio_repr {
        media_info_keys.push(match &audio_repr {
            CreatedAudioRepr::Transcode(transcode) => transcode.out_media_info_key.clone(),
            CreatedAudioRepr::PackagedOriginalFile {
                out_file_key: _,
                out_media_info_key,
            } => out_media_info_key.clone(),
        });
    }
    let mpd_key = storage_key::mpd_manifest(asset_id);
    MpdGenerator::run(
        media_info_keys.iter().map(AsRef::as_ref),
        &mpd_key,
        storage,
        mpd_generator_path,
    )
    .await
    .wrap_err("could not generate mpd manifest")?;
    Ok(CompletedPackageVideo {
        asset_id,
        created_video_repr,
        created_audio_repr,
    })
}
