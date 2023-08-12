use std::path::PathBuf;

use eyre::{eyre, Context, Report, Result};

use crate::processing::video::dash_package::RepresentationInput;
use crate::processing::video::probe_video;
use crate::{
    catalog::{
        encoding_target::{codec_name, EncodingTarget},
        operation::resource_path_on_disk,
        PathInResourceDir, ResolvedResourcePath, ResourcePath,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetId, AudioRepresentation, AudioRepresentationId, Size, Video, VideoAsset,
        VideoRepresentation, VideoRepresentationId,
    },
    processing::video::transcode::ffmpeg_command,
};

/// Package video asset for DASH.
/// If transcode is set, ffmpeg to target codec.
/// Then gather existing representations and pass it all to shaka-packager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVideo<P: ResourcePath> {
    pub asset_id: AssetId,
    pub output_dir: P,
    pub transcode: Option<Transcode>,
    pub mpd_output: PathBuf,
}

// Some things like the resulting size and bitrate of
// a video we don't actually know until ffmpeg is done.
// That information needs to be known to apply the operation
// to the database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedPackageVideo {
    pub asset_id: AssetId,
    pub output_dir: ResolvedResourcePath,
    /// relative to output_dir
    pub mpd_output: PathBuf,
    pub transcode_result: Option<TranscodeResult>,
    /// relative to output_dir
    pub audio_output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcode {
    pub target: EncodingTarget,
    /// output path where the final transcoded and shaka remuxed video file should be
    /// relative to PackageVideo::output_dir
    pub output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscodeResult {
    pub target: EncodingTarget,
    pub final_size: Size,
    pub bitrate: i64,
    /// relative to video resource_dir
    pub output: PathBuf,
}

pub async fn apply_package_video(pool: &DbPool, op: &CompletedPackageVideo) -> Result<()> {
    let asset: VideoAsset = repository::asset::get_asset(pool, op.asset_id)
        .await?
        .try_into()?;
    let mut tx = pool
        .begin()
        .await
        .wrap_err("could not begin db transaction")?;
    let asset = match &op.output_dir {
        ResolvedResourcePath::Existing(resource_dir) => {
            assert!(asset.video.dash_resource_dir.unwrap() == resource_dir.resource_dir_id);
            asset
        }
        ResolvedResourcePath::New(resource_dir) => {
            assert!(asset.video.dash_resource_dir.is_none());
            let resource_dir_id = repository::resource_file::insert_new_resource_file2(
                tx.as_mut(),
                resource_dir.data_dir_id,
                &resource_dir.path_in_data_dir,
            )
            .await?;
            VideoAsset {
                video: Video {
                    dash_resource_dir: Some(resource_dir_id),
                    ..asset.video
                },
                ..asset
            }
        }
    };
    if let Some(audio_output) = &op.audio_output {
        let audio_representation = AudioRepresentation {
            id: AudioRepresentationId(0),
            asset_id: op.asset_id,
            path_in_resource_dir: audio_output.clone(),
        };
        let _audio_representation_id = repository::representation::insert_audio_representation(
            tx.as_mut(),
            audio_representation,
        )
        .await?;
    }
    if let Some(transcode) = &op.transcode_result {
        let size = match transcode.target.scale {
            Some(scale) => todo!(),
            None => asset.base.size,
        };
        let video_represention = VideoRepresentation {
            id: VideoRepresentationId(0),
            asset_id: asset.base.id,
            codec_name: codec_name(&transcode.target.codec),
            width: transcode.final_size.width,
            height: transcode.final_size.height,
            bitrate: transcode.bitrate,
            path_in_resource_dir: transcode.output.clone(),
        };
        let _representation_id = repository::representation::insert_video_representation(
            tx.as_mut(),
            video_represention,
        )
        .await?;
    }
    tx.commit()
        .await
        .wrap_err("could not commit db transaction")?;
    todo!()
}

pub struct PackageVideoSideEffectResult {
    failed: Vec<(PackageVideo<PathInResourceDir>, Report)>,
}

pub async fn perform_side_effects_package_video(
    pool: &DbPool,
    op: &PackageVideo<ResolvedResourcePath>,
) -> Result<CompletedPackageVideo> {
    // create directories
    let output_dir = resource_path_on_disk(pool, &op.output_dir).await?;
    tokio::fs::create_dir_all(&output_dir)
        .await
        .wrap_err("could not create output directory")?;
    let asset: VideoAsset = repository::asset::get_asset(pool, op.asset_id)
        .await?
        .try_into()?;
    let asset_path = repository::asset::get_asset_path_on_disk(pool, op.asset_id).await?;
    // TODO ffmpeg should actually output to temp file
    // ouput path in transcode_result needs to be the shaka remuxed file
    let transcode_result: Option<TranscodeResult> = match &op.transcode {
        Some(transcode) => {
            let output_path = output_dir.join(&transcode.output);
            let mut command =
                ffmpeg_command(&asset_path.path_on_disk(), &output_path, &transcode.target);
            let ffmpeg_result = command.spawn()?.wait().await?;
            if !ffmpeg_result.success() {
                return Err(eyre!("ffmpeg exited with an error"));
            }
            let probe = probe_video(&output_path).await?;
            Some(TranscodeResult {
                target: transcode.target.clone(),
                final_size: Size {
                    width: probe.width,
                    height: probe.height,
                },
                bitrate: probe.bitrate,
                output: output_path,
            })
        }
        None => None,
    };
    // if transcode: ffmpeg
    // shaka-packager
    let reprs: [RepresentationInput] = [];
    todo!()
}
