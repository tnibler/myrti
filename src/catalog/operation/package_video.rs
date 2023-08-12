use eyre::{Report, Result};

use crate::{
    catalog::{
        encoding_target::EncodingTarget, PathInResourceDir, ResolvedResourcePath, ResourcePath,
    },
    model::{repository::pool::DbPool, AssetId},
};

/// Package video asset for DASH.
/// If transcode is set, ffmpeg to target codec.
/// Then gather existing representations and pass it all to shaka-packager.
#[derive(Debug, Clone)]
pub struct PackageVideo<P: ResourcePath> {
    pub asset_id: AssetId,
    pub transcode: Option<Transcode<P>>,
    pub mpd_output: P,
}

#[derive(Debug, Clone)]
pub struct Transcode<P: ResourcePath> {
    pub target: EncodingTarget,
    /// output path where the final transcoded and shaka remuxed video file should be
    pub output: P,
}

pub async fn apply_package_video(
    pool: &DbPool,
    op: &PackageVideo<ResolvedResourcePath>,
) -> Result<()> {
    todo!()
}

pub struct PackageVideoSideEffectResult {
    failed: Vec<(PackageVideo<PathInResourceDir>, Report)>,
}

pub async fn perform_side_effects_package_video(
    pool: &DbPool,
    op: &PackageVideo<ResolvedResourcePath>,
) -> Result<PackageVideoSideEffectResult> {
    todo!()
}
