use std::path::PathBuf;

use eyre::{eyre, Context, Report, Result};
use tracing::{info, instrument, warn};

use crate::processing::video::dash_package::{
    generate_mpd, shaka_package, RepresentationInput, RepresentationType,
};
use crate::processing::video::probe_video;
use crate::{
    catalog::encoding_target::{codec_name, EncodingTarget},
    model::{
        repository::{self, pool::DbPool},
        AssetId, AudioRepresentation, AudioRepresentationId, Size, Video, VideoAsset,
        VideoRepresentation, VideoRepresentationId,
    },
    processing::video::transcode::ffmpeg_command,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateAudioRepr {
    Existing(AudioRepresentation),
    CreateNew(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateVideoRepr {
    PackageOriginalFile(PathBuf),
    Transcode(Transcode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// all paths are relative to the dash resource directory
pub struct PackageVideo {
    pub asset_id: AssetId,
    pub create_video_repr: CreateVideoRepr,
    pub create_audio_repr: CreateAudioRepr,
    pub existing_video_reprs: Vec<VideoRepresentation>,
    pub mpd_output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVideoWithPath {
    pub output_dir: PathBuf,
    pub package_video: PackageVideo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatedVideoRepr {
    /// absolute path
    PackagedOriginalFile(PathBuf),
    Transcode(TranscodeResult),
}

// Some things like the resulting size and bitrate of
// a video we don't actually know until ffmpeg is done.
// That information needs to be known to apply the operation
// to the database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedPackageVideo {
    pub asset_id: AssetId,
    pub output_dir: PathBuf,
    pub created_video_repr: CreatedVideoRepr,
    pub created_audio_repr: CreateAudioRepr,
    /// relative to output_dir
    pub mpd_output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcode {
    pub target: EncodingTarget,
    /// path where the final transcoded and shaka remuxed video file should be
    /// relative to PackageVideo::output_dir
    pub output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscodeResult {
    pub target: EncodingTarget,
    pub final_size: Size,
    pub bitrate: i64,
    /// absolute path
    pub output: PathBuf,
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
            dash_resource_dir: Some(op.output_dir.clone()),
            ..asset.video
        },
        ..asset
    };
    repository::asset::update_asset(tx.as_mut(), &(&asset).into()).await?;
    match &op.created_audio_repr {
        CreateAudioRepr::CreateNew(audio_output) => {
            let audio_representation = AudioRepresentation {
                id: AudioRepresentationId(0),
                asset_id: op.asset_id,
                path: op.output_dir.join(audio_output),
            };
            let _audio_representation_id = repository::representation::insert_audio_representation(
                tx.as_mut(),
                audio_representation,
            )
            .await?;
        }
        _ => {}
    }
    let video_represention = match &op.created_video_repr {
        CreatedVideoRepr::PackagedOriginalFile(video_output) => VideoRepresentation {
            id: VideoRepresentationId(0),
            asset_id: asset.base.id,
            codec_name: asset.video.codec_name,
            width: asset.base.size.width,
            height: asset.base.size.height,
            bitrate: asset.video.bitrate,
            path: op.output_dir.join(video_output),
        },
        CreatedVideoRepr::Transcode(transcode) => VideoRepresentation {
            id: VideoRepresentationId(0),
            asset_id: asset.base.id,
            codec_name: codec_name(&transcode.target.codec),
            width: transcode.final_size.width,
            height: transcode.final_size.height,
            bitrate: transcode.bitrate,
            path: op.output_dir.join(&transcode.output),
        },
    };
    let _representation_id =
        repository::representation::insert_video_representation(tx.as_mut(), video_represention)
            .await?;
    tx.commit()
        .await
        .wrap_err("could not commit db transaction")?;
    Ok(())
}

pub struct PackageVideoSideEffectResult {
    failed: Vec<(PackageVideo, Report)>,
}

#[instrument(skip(pool))]
pub async fn perform_side_effects_package_video(
    pool: &DbPool,
    op: &PackageVideoWithPath,
) -> Result<CompletedPackageVideo> {
    // create directories
    tokio::fs::create_dir_all(&op.output_dir)
        .await
        .wrap_err("could not create output directory")?;
    let package_video = &op.package_video;
    let output_dir = &op.output_dir;
    let asset_id = package_video.asset_id;
    let asset: VideoAsset = repository::asset::get_asset(pool, asset_id)
        .await?
        .try_into()?;
    let asset_path = repository::asset::get_asset_path_on_disk(pool, asset_id).await?;

    let audio_path = match &package_video.create_audio_repr {
        CreateAudioRepr::Existing(audio_repr) => audio_repr.path.clone(),
        CreateAudioRepr::CreateNew(audio_output) => {
            let out_path = output_dir.join(audio_output);
            let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Audio,
                out_path: out_path.clone(),
            }];
            shaka_package(repr_inputs)
                .await
                .wrap_err("could not shaka package audio stream")?;
            out_path
        }
    };

    let created_video_repr = match &package_video.create_video_repr {
        CreateVideoRepr::PackageOriginalFile(video_output) => {
            let out_path = output_dir.join(video_output);
            let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Video,
                out_path: out_path.clone(),
            }];
            shaka_package(repr_inputs)
                .await
                .wrap_err("could not shaka package video stream")?;
            CreatedVideoRepr::PackagedOriginalFile(out_path)
        }
        CreateVideoRepr::Transcode(transcode) => {
            // ffmpeg transcodes to a temp file which is then fed through shaka packager
            let temp_dir = tempfile::tempdir().wrap_err("error creating temp directory")?;
            let ffmpeg_out_path = temp_dir.path().join("out.mp4");
            let mut command = ffmpeg_command(
                &asset_path.path_on_disk(),
                &ffmpeg_out_path,
                &transcode.target,
            );
            let ffmpeg_result = command.spawn()?.wait().await?;
            if !ffmpeg_result.success() {
                return Err(eyre!("ffmpeg exited with an error"));
            }
            let output_path = output_dir.join(&transcode.output);
            let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                path: ffmpeg_out_path,
                ty: RepresentationType::Video,
                out_path: output_path.clone(),
            }];
            shaka_package(repr_inputs)
                .await
                .wrap_err("could not shaka package video stream")?;
            let probe = probe_video(&output_path).await?;
            CreatedVideoRepr::Transcode(TranscodeResult {
                target: transcode.target.clone(),
                final_size: Size {
                    width: probe.width,
                    height: probe.height,
                },
                bitrate: probe.bitrate,
                output: output_path,
            })
        }
    };

    let mut media_infos_relative: Vec<PathBuf> = Vec::default();
    for video_repr in &package_video.existing_video_reprs {
        // shaka packager generates video1.mp4.media_info for output video1.mp4
        let mut path = video_repr.path.clone();
        let filename = match video_repr.path.file_name() {
            Some(s) => s.to_str().unwrap(),
            None => {
                warn!(
                    repr_path = %video_repr.path.display(),
                    "No media_info for video representation"
                );
                continue;
            }
        };
        path.set_file_name(format!("{}.media_info", filename));
        media_infos_relative.push(path);
    }
    let created_video_repr_path = match &created_video_repr {
        CreatedVideoRepr::PackagedOriginalFile(video_path) => video_path.as_path(),
        CreatedVideoRepr::Transcode(transcode_result) => transcode_result.output.as_path(),
    };
    match created_video_repr_path.file_name() {
        Some(filename) => {
            let mut path = created_video_repr_path.to_owned();
            path.set_file_name(format!("{}.media_info", filename.to_str().unwrap()));
            // path is actually absolute here, but since joining an absolute path just overwrites
            // it's fine
            media_infos_relative.push(path);
        }
        None => {
            warn!(
                repr_path = %created_video_repr_path.display(),
                "No media_info for video representation"
            );
        }
    };
    match &audio_path.file_name() {
        Some(filename) => {
            media_infos_relative.push(PathBuf::from(format!(
                "{}.media_info",
                filename.to_str().unwrap()
            )));
        }
        None => {
            warn!(
                repr_path = %audio_path.display(),
                "No media_info for audio representation"
            );
        }
    };
    let media_infos_abs: Vec<PathBuf> = media_infos_relative
        .into_iter()
        .map(|path| output_dir.join(path))
        .collect();
    generate_mpd(
        &media_infos_abs,
        output_dir.join(&package_video.mpd_output).as_path(),
    )
    .await
    .wrap_err("could not generate mpd manifest")?;
    Ok(CompletedPackageVideo {
        asset_id,
        output_dir: op.output_dir.clone(),
        mpd_output: package_video.mpd_output.clone(),
        created_video_repr,
        created_audio_repr: package_video.create_audio_repr.clone(),
    })
}
