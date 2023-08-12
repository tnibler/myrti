use std::path::PathBuf;

use eyre::{Context, Report, Result};

use crate::{
    catalog::{
        encoding_target::{codec_name, EncodingTarget},
        PathInResourceDir, ResolvedResourcePath, ResourcePath,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetId, Video, VideoAsset, VideoRepresentation, VideoRepresentationId,
    },
};

/// Package video asset for DASH.
/// If transcode is set, ffmpeg to target codec.
/// Then gather existing representations and pass it all to shaka-packager.
#[derive(Debug, Clone)]
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
pub struct CompletePackageVideoSideEffects {
    pub task: PackageVideo<ResolvedResourcePath>,
}

#[derive(Debug, Clone)]
pub struct Transcode {
    pub target: EncodingTarget,
    /// output path where the final transcoded and shaka remuxed video file should be
    /// relative to PackageVideo::output_dir
    pub output: PathBuf,
}

pub async fn apply_package_video(
    pool: &DbPool,
    op: &PackageVideo<ResolvedResourcePath>,
) -> Result<()> {
    // if resource_dir for asset is not set, set it
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
    if let Some(transcode) = op.transcode {
        let size = match transcode.target.scale {
            Some(scale) => todo!(),
            None => asset.base.size,
        };
        let video_represention = VideoRepresentation {
            id: VideoRepresentationId(0),
            asset_id: asset.base.id,
            codec_name: codec_name(transcode.target.codec),
            width: transcode.target.scale,
            height: todo!(),
            bitrate: todo!(),
            path_in_resource_dir: todo!(),
        };
    }
    // if transcode: insert new representation
    todo!()
}

pub struct PackageVideoSideEffectResult {
    failed: Vec<(PackageVideo<PathInResourceDir>, Report)>,
}

pub async fn perform_side_effects_package_video(
    pool: &DbPool,
    op: &PackageVideo<ResolvedResourcePath>,
) -> Result<PackageVideoSideEffectResult> {
    // if transcode: ffmpeg
    // create directories
    // shaka-packager
    todo!()
}
