use eyre::{Context, Result};

use crate::{
    catalog::image_conversion_target::{image_format_name, ImageConversionTarget},
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    interact,
    model::{
        repository::{
            self,
            db::{DbPool, PooledDbConn},
        },
        AssetId, ImageRepresentation, ImageRepresentationId, Size,
    },
    processing::{self, image::image_conversion::ConvertImageTrait},
};

#[derive(Debug, Clone)]
pub struct ConvertImage {
    pub asset_id: AssetId,
    pub target: ImageConversionTarget,
    pub output_file_key: String,
}

#[tracing::instrument(skip(conn), level = "debug")]
pub async fn apply_convert_image(
    conn: &mut PooledDbConn,
    op: &ConvertImage,
    result: ImageConversionSideEffectResult,
) -> Result<()> {
    let image_representation = ImageRepresentation {
        id: ImageRepresentationId(0),
        asset_id: op.asset_id,
        format_name: image_format_name(&op.target.format).to_owned(),
        file_key: op.output_file_key.clone(),
        file_size: result.file_size,
        width: result.final_size.width,
        height: result.final_size.height,
    };
    interact!(conn, move |conn| {
        repository::representation::insert_image_representation(conn, &image_representation)
            .wrap_err("error inserting image representation")
    })
    .await??;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageConversionSideEffectResult {
    pub final_size: Size,
    pub file_size: i64,
}

#[tracing::instrument(skip(storage, pool), level = "debug")]
pub async fn perform_side_effects_convert_image(
    op: &ConvertImage,
    pool: DbPool,
    storage: &Storage,
) -> Result<ImageConversionSideEffectResult> {
    let command_out_file = storage.new_command_out_file(&op.output_file_key).await?;
    let conn = pool.get().await?;
    let asset_id = op.asset_id;
    let (asset, asset_path) = interact!(conn, move |conn| {
        // FIXME (low) unnecessarily querying same row twice
        let asset = repository::asset::get_asset(conn, asset_id)?;
        let asset_path = repository::asset::get_asset_path_on_disk(conn, asset_id)?;
        Ok((asset, asset_path))
    })
    .await??;
    let scaled_size = processing::image::image_conversion::ConvertImage::convert_image(
        asset_path.path_on_disk(),
        op.target.clone(),
        &op.output_file_key,
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
