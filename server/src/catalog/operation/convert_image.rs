use eyre::Result;

use crate::{
    catalog::image_conversion_target::ImageConversionTarget,
    core::storage::Storage,
    model::{repository::pool::DbPool, AssetId},
};

#[derive(Debug, Clone)]
pub struct ConvertImage {
    pub asset_id: AssetId,
    pub target: ImageConversionTarget,
    pub output_key: String,
}

#[tracing::instrument(skip(pool))]
pub async fn apply_convert_image(pool: &DbPool, op: &ConvertImage) -> Result<()> {
    todo!()
}

#[tracing::instrument(skip(storage))]
pub async fn perform_side_effects_convert_image(
    op: &ConvertImage,
    pool: &DbPool,
    storage: &Storage,
) -> Result<()> {
    todo!()
}
