use eyre::{Context, Result};

use crate::{
    catalog::image_conversion_target::{image_format_name, ImageConversionTarget},
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    model::{
        repository::{self, pool::DbPool},
        AssetId, ImageRepresentation, ImageRepresentationId, Size,
    },
    processing::{self, image::image_conversion::ConvertImageTrait},
};

#[derive(Debug, Clone)]
pub struct ConvertImage {
    pub asset_id: AssetId,
    pub target: ImageConversionTarget,
    pub output_key: String,
}

#[tracing::instrument(skip(pool))]
pub async fn apply_convert_image(pool: &DbPool, op: &ConvertImage, final_size: Size) -> Result<()> {
    let image_representation = ImageRepresentation {
        id: ImageRepresentationId(0),
        asset_id: op.asset_id,
        format_name: image_format_name(&op.target.format).to_owned(),
        file_key: op.output_key.clone(),
        width: final_size.width,
        height: final_size.height,
    };
    repository::representation::insert_image_representation(pool, &image_representation)
        .await
        .wrap_err("error inserting image representation")?;
    Ok(())
}

#[tracing::instrument(skip(storage))]
pub async fn perform_side_effects_convert_image(
    op: &ConvertImage,
    pool: &DbPool,
    storage: &Storage,
) -> Result<Size> {
    let command_out_file = storage.new_command_out_file(&op.output_key).await?;
    // FIXME (low) unnecessarily querying same row twice
    let asset = repository::asset::get_asset(pool, op.asset_id).await?;
    let asset_path = repository::asset::get_asset_path_on_disk(pool, op.asset_id)
        .await?
        .path_on_disk();
    let scaled_size = processing::image::image_conversion::ConvertImage::convert_image(
        asset_path,
        op.target.clone(),
        &op.output_key,
        storage,
    )
    .await
    .wrap_err("error converting image")?;
    command_out_file.flush_to_storage().await?;
    Ok(scaled_size.unwrap_or(asset.base.size))
}
