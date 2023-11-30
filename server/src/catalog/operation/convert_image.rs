use eyre::{Context, Result};

use crate::{
    catalog::{
        image_conversion_target::{image_format_name, ImageConversionTarget},
        storage_key,
    },
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
}

#[derive(Debug, Clone)]
pub struct ConvertImagePrereserveResult {
    pub image_representation_id: ImageRepresentationId,
    pub output_file_key: String,
}

#[tracing::instrument(skip(pool))]
pub async fn preallocate_dummy_image_repr_rows(
    pool: &DbPool,
    op: &ConvertImage,
) -> Result<ConvertImagePrereserveResult> {
    let repr_id = repository::representation::reserve_image_representation(
        pool,
        op.asset_id,
        image_format_name(&op.target.format),
    )
    .await?;
    let file_key = storage_key::image_representation(
        op.asset_id,
        repr_id,
        storage_key::image_file_extension(&op.target.format),
    );
    Ok(ConvertImagePrereserveResult {
        image_representation_id: repr_id,
        output_file_key: file_key,
    })
}

#[tracing::instrument(skip(pool))]
pub async fn apply_convert_image(
    pool: &DbPool,
    op: &ConvertImage,
    reserved: &ConvertImagePrereserveResult,
    result: ImageConversionSideEffectResult,
) -> Result<()> {
    let image_representation = ImageRepresentation {
        id: reserved.image_representation_id,
        asset_id: op.asset_id,
        format_name: image_format_name(&op.target.format).to_owned(),
        file_key: reserved.output_file_key.clone(),
        file_size: result.file_size,
        width: result.final_size.width,
        height: result.final_size.height,
    };
    repository::representation::update_dummy_image_representation(pool, &image_representation)
        .await
        .wrap_err("error updating dummy image representation")?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageConversionSideEffectResult {
    pub final_size: Size,
    pub file_size: i64,
}

#[tracing::instrument(skip(storage))]
pub async fn perform_side_effects_convert_image(
    reserved: &ConvertImagePrereserveResult,
    op: &ConvertImage,
    pool: &DbPool,
    storage: &Storage,
) -> Result<ImageConversionSideEffectResult> {
    let command_out_file = storage
        .new_command_out_file(&reserved.output_file_key)
        .await?;
    // FIXME (low) unnecessarily querying same row twice
    let asset = repository::asset::get_asset(pool, op.asset_id).await?;
    let asset_path = repository::asset::get_asset_path_on_disk(pool, op.asset_id)
        .await?
        .path_on_disk();
    let scaled_size = processing::image::image_conversion::ConvertImage::convert_image(
        asset_path,
        op.target.clone(),
        &reserved.output_file_key,
        storage,
    )
    .await
    .wrap_err("error converting image")?;
    let file_size = command_out_file.size().await?;
    command_out_file.flush_to_storage().await?;
    Ok(ImageConversionSideEffectResult {
        final_size: scaled_size.unwrap_or(asset.base.size),
        file_size: file_size as i64,
    })
}
