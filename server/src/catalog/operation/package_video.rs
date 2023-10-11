use std::ffi::OsString;

use camino::Utf8PathBuf as PathBuf;
use eyre::{eyre, Context, Result};
use tracing::{error, instrument, Instrument};

use crate::{
    catalog::{
        encoding_target::{audio_codec_name, codec_name, VideoEncodingTarget},
        storage_key,
    },
    core::storage::Storage,
    model::{
        repository::{self, pool::DbPool},
        AssetId, AudioRepresentation, AudioRepresentationId, Size, Video, VideoAsset,
        VideoRepresentation, VideoRepresentationId,
    },
    processing::{
        commands::{FFmpeg, FFmpegIntoShaka, MpdGenerator, ShakaIntoFFmpeg, ShakaPackager},
        video::{
            ffmpeg::FFmpegTrait,
            ffmpeg_into_shaka::{FFmpegIntoShakaFFmpegTrait, FFmpegIntoShakaTrait},
            mpd_generator::MpdGeneratorTrait,
            shaka::{RepresentationType, ShakaPackagerTrait},
            shaka_into_ffmpeg::ShakaIntoFFmpegTrait,
            transcode::{ProduceAudio, ProduceVideo},
            video_rotation::FFProbeRotationTrait,
            FFProbe,
        },
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateAudioRepr {
    Existing(AudioRepresentation),
    Transcode(AudioTranscode),
    PackageOriginalFile { output_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatedAudioRepr {
    Existing(AudioRepresentation),
    Transcode(AudioTranscodeResult),
    PackagedOriginalFile {
        out_file_key: String,
        out_media_info_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateVideoRepr {
    Existing(VideoRepresentation),
    Transcode(VideoTranscode),
    PackageOriginalFile { output_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// all paths are relative to the dash resource directory
pub struct PackageVideo {
    pub asset_id: AssetId,
    pub create_video_repr: CreateVideoRepr,
    pub create_audio_repr: Option<CreateAudioRepr>,
    pub existing_video_reprs: Vec<VideoRepresentation>,
    pub mpd_out_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVideoWithPath {
    pub output_dir: PathBuf,
    pub package_video: PackageVideo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatedVideoRepr {
    PackagedOriginalFile {
        out_file_key: String,
        out_media_info_key: String,
    },
    Transcode(VideoTranscodeResult),
    Existing(VideoRepresentation),
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

#[instrument(skip(pool))]
pub async fn apply_package_video(pool: &DbPool, op: &CompletedPackageVideo) -> Result<()> {
    let asset: VideoAsset = repository::asset::get_asset(pool, op.asset_id)
        .await?
        .try_into()?;
    let mut tx = pool
        .begin()
        .await
        .wrap_err("could not begin db transaction")?;
    let asset = VideoAsset {
        video: Video {
            has_dash: true,
            ..asset.video
        },
        ..asset
    };
    repository::asset::set_asset_has_dash(tx.as_mut(), op.asset_id, true).await?;
    match &op.created_audio_repr {
        Some(CreatedAudioRepr::Transcode(audio_transcode)) => {
            let audio_representation = AudioRepresentation {
                id: AudioRepresentationId(0),
                asset_id: op.asset_id,
                codec_name: audio_codec_name(&audio_transcode.target),
                file_key: audio_transcode.out_file_key.clone(),
                media_info_key: audio_transcode.out_media_info_key.clone(),
            };
            let _audio_representation_id = repository::representation::insert_audio_representation(
                tx.as_mut(),
                &audio_representation,
            )
            .await?;
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
            let _audio_representation_id = repository::representation::insert_audio_representation(
                tx.as_mut(),
                &audio_representation,
            )
            .await?;
        }
        None | Some(CreatedAudioRepr::Existing(_)) => {}
    }
    let created_video_represention = match &op.created_video_repr {
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
            codec_name: codec_name(&transcode.target.codec),
            width: transcode.final_size.width,
            height: transcode.final_size.height,
            bitrate: transcode.bitrate,
            file_key: transcode.out_file_key.clone(),
            media_info_key: transcode.out_media_info_key.clone(),
        }),
        CreatedVideoRepr::Existing(_) => None,
    };
    if let Some(video_represention) = created_video_represention {
        let _representation_id = repository::representation::insert_video_representation(
            tx.as_mut(),
            &video_represention,
        )
        .await?;
    }
    tx.commit()
        .await
        .wrap_err("could not commit db transaction")?;
    Ok(())
}

#[instrument(skip(pool, storage))]
pub async fn perform_side_effects_package_video(
    pool: &DbPool,
    storage: &Storage,
    package_video: &PackageVideo,
) -> Result<CompletedPackageVideo> {
    let asset_id = package_video.asset_id;
    let asset_path = repository::asset::get_asset_path_on_disk(pool, asset_id).await?;

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
        Some(ffmpeg_into_shaka.run_ffmpeg(Some("ffmpeg")).await?)
    } else {
        None
    };

    let created_audio_repr: Option<CreatedAudioRepr> = match &package_video.create_audio_repr {
        Some(CreateAudioRepr::Existing(audio_repr)) => {
            Some(CreatedAudioRepr::Existing(audio_repr.clone()))
        }
        Some(CreateAudioRepr::PackageOriginalFile { output_key }) => {
            ShakaPackager::run(
                &asset_path.path_on_disk(),
                RepresentationType::Audio,
                &output_key,
                storage,
                Some("/home/thomas/p/mediathingy/shaka-bin/packager"),
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
                    &storage,
                    Some("/home/thomas/p/mediathingy/shaka-bin/packager"),
                )
                .in_current_span()
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
        CreateVideoRepr::Existing(video_repr) => CreatedVideoRepr::Existing(video_repr.clone()),
        CreateVideoRepr::PackageOriginalFile { output_key } => {
            // shaka-packager discards some metadata, notable stream side data like
            // rotation.
            // To correct that, we have to rerun the shaka-packager output through ffmpeg
            // to set the rotation again if present.
            // BUT since shaka-packager also outputs a media_info file that we need,
            // the shaka-packager output filename needs to be the same as the final ffmpeg
            // output.
            // TODO calling ffprobe yet again, ideally once is enough? Or not I'm not sure
            let rotation =
                FFProbe::video_rotation(&asset_path.path_on_disk(), Some("ffprobe")).await?;
            if let Some(rotation) = rotation {
                let pre_input_flags: Vec<OsString> =
                    vec!["-display_rotation".into(), rotation.to_string().into()];
                let flags: Vec<OsString> = vec!["-c:v".into(), "copy".into()];
                let correct_rotation_ffmpeg: FFmpeg = FFmpeg::new(pre_input_flags, flags);
                let shaka_result = ShakaIntoFFmpeg::run(
                    &asset_path.path_on_disk(),
                    RepresentationType::Video,
                    &correct_rotation_ffmpeg,
                    output_key,
                    storage,
                    Some("/home/thomas/p/mediathingy/shaka-bin/packager"),
                    Some("ffmpeg"),
                )
                .await?;

                CreatedVideoRepr::PackagedOriginalFile {
                    out_file_key: output_key.clone(),
                    out_media_info_key: shaka_result.media_info_key,
                }
            } else {
                let shaka_result = ShakaPackager::run(
                    &asset_path.path_on_disk(),
                    RepresentationType::Video,
                    output_key,
                    storage,
                    Some("/home/thomas/p/mediathingy/shaka-bin/packager"),
                )
                .await
                .wrap_err("could not shaka package audio stream")?;
                CreatedVideoRepr::PackagedOriginalFile {
                    out_file_key: output_key.clone(),
                    out_media_info_key: shaka_result.media_info_key,
                }
            }
        }
        CreateVideoRepr::Transcode(transcode) => {
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
                    RepresentationType::Video,
                    &transcode.output_key,
                    &storage,
                    Some("/home/thomas/p/mediathingy/shaka-bin/packager"),
                )
                .in_current_span()
                .await?;
            let probe = ffmpeg_into_shaka
                .ffprobe_get_streams(Some("ffprobe"))
                .await?
                .video;
            CreatedVideoRepr::Transcode(VideoTranscodeResult {
                target: transcode.target.clone(),
                final_size: Size {
                    width: probe.width,
                    height: probe.height,
                },
                bitrate: probe.bitrate,
                out_file_key: transcode.output_key.clone(),
                out_media_info_key: shaka_result.media_info_key,
            })
        }
    };

    // mpd_generator needs media_infos as local files
    // We just copy the
    let mut media_info_keys: Vec<String> = Vec::default();
    media_info_keys.push(match &created_video_repr {
        CreatedVideoRepr::Existing(repr) => repr.media_info_key.clone(),
        CreatedVideoRepr::Transcode(transcode) => transcode.out_media_info_key.clone(),
        CreatedVideoRepr::PackagedOriginalFile {
            out_file_key: _,
            out_media_info_key,
        } => out_media_info_key.clone(),
    });
    if let Some(audio_repr) = &created_audio_repr {
        media_info_keys.push(match &audio_repr {
            CreatedAudioRepr::Existing(repr) => repr.media_info_key.clone(),
            CreatedAudioRepr::Transcode(transcode) => transcode.out_media_info_key.clone(),
            CreatedAudioRepr::PackagedOriginalFile {
                out_file_key: _,
                out_media_info_key,
            } => out_media_info_key.clone(),
        });
    }
    for video_repr in &package_video.existing_video_reprs {
        media_info_keys.push(video_repr.media_info_key.clone());
    }
    let mpd_key = storage_key::mpd_manifest(asset_id);
    MpdGenerator::run(
        media_info_keys.iter().map(AsRef::as_ref),
        &mpd_key,
        storage,
        Some("/home/thomas/p/mediathingy/shaka-bin/mpd_generator"),
    )
    .await
    .wrap_err("could not generate mpd manifest")?;
    Ok(CompletedPackageVideo {
        asset_id,
        created_video_repr,
        created_audio_repr,
    })
}
