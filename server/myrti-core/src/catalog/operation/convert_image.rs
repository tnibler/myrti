use eyre::{Context, Result};

use myrti_data::db::{DbPool, PooledDbConn};
use myrti_data::model::{FileId, ImageRepresentation, ImageRepresentationId, Size};
use myrti_data::{interact, repository};

use crate::{
    catalog::image_conversion_target::{ImageConversionTarget, image_format_name},
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    processing::{self, image::image_conversion::ConvertImageTrait},
};

#[derive(Debug, Clone)]
pub struct ConvertImage {
    pub file_id: FileId,
    pub target: ImageConversionTarget,
    pub output_file_key: String,
}

pub async fn apply_convert_image(
    conn: &mut PooledDbConn,
    op: &ConvertImage,
    result: ImageConversionSideEffectResult,
) -> Result<()> {
    let image_representation = ImageRepresentation {
        id: ImageRepresentationId(0),
        file_id: op.file_id,
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

pub async fn perform_side_effects_convert_image(
    op: &ConvertImage,
    pool: DbPool,
    storage: &Storage,
) -> Result<ImageConversionSideEffectResult> {
    let command_out_file = storage.new_command_out_file(&op.output_file_key).await?;
    let conn = pool.get().await?;
    let file_id = op.file_id;
    let (file, asset_path) = interact!(conn, move |conn| {
        // FIXME (low) unnecessarily querying same row twice
        let file = repository::asset::get_asset_file(conn, file_id)?;
        let asset_path = repository::asset::get_asset_path_on_disk(conn, file_id)?;
        Ok((file, asset_path))
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
        final_size: scaled_size.unwrap_or(file.size),
        file_size: file_size as i64,
    })
}
