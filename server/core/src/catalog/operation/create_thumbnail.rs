use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Report, Result};
use tracing::{instrument, Instrument};

use crate::{
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    interact,
    model::{
        repository::{self, db::DbPool},
        AssetId, AssetType, Size, ThumbnailFormat, ThumbnailType,
    },
    processing::{
        self,
        commands::GenerateThumbnail,
        image::thumbnail::{GenerateThumbnailTrait, ThumbnailParams},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateThumbnail {
    pub asset_id: AssetId,
    pub thumbnails: Vec<ThumbnailToCreate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreate {
    pub ty: ThumbnailType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateThumbnailWithPaths {
    pub asset_id: AssetId,
    pub thumbnails: Vec<ThumbnailToCreateWithPaths>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreateWithPaths {
    pub ty: ThumbnailType,
    pub avif_key: String,
    pub webp_key: String,
}

#[instrument(skip(pool), level = "debug")]
pub async fn apply_create_thumbnail(pool: DbPool, op: CreateThumbnailWithPaths) -> Result<()> {
    let conn = pool.get().in_current_span().await?;
    interact!(conn, move |mut conn| {
        for thumb in &op.thumbnails {
            // we never query thumbnail sizes and don't have access to them right here so ignore
            // for now
            let size = Size {
                width: 0,
                height: 0,
            };
            repository::asset::set_asset_has_thumbnail(
                &mut conn,
                op.asset_id,
                thumb.ty,
                size,
                &[ThumbnailFormat::Avif, ThumbnailFormat::Webp],
            )?;
        }
        Ok(())
    })
    .in_current_span()
    .await??;
    Ok(())
}

pub struct ThumbnailSideEffectResult {
    pub failed: Vec<(ThumbnailToCreateWithPaths, Report)>,
}

#[instrument(skip(pool, storage), level = "debug")]
pub async fn perform_side_effects_create_thumbnail(
    storage: &Storage,
    pool: DbPool,
    op: CreateThumbnailWithPaths,
) -> Result<ThumbnailSideEffectResult> {
    let mut result = ThumbnailSideEffectResult {
        failed: Vec::default(),
    };
    if op.thumbnails.is_empty() {
        return Ok(result);
    }
    let conn = pool.get().in_current_span().await?;
    let (in_path, asset) = interact!(conn, move |mut conn| {
        let in_path =
            repository::asset::get_asset_path_on_disk(&mut conn, op.asset_id)?.path_on_disk();
        let asset = repository::asset::get_asset(&mut conn, op.asset_id)?;
        Ok::<_, eyre::Report>((in_path, asset))
    })
    .await??;
    // TODO don't await sequentially. Not super bad because op.thumbnails is small but still
    for thumb in &op.thumbnails {
        match create_thumbnail(in_path.clone(), asset.base.ty, thumb, storage)
            .in_current_span()
            .await
        {
            Ok(_) => (),
            Err(err) => {
                result.failed.push((thumb.clone(), err));
            }
        }
    }
    Ok(result)
}

#[instrument(skip(storage), level = "debug")]
async fn create_thumbnail(
    asset_path: PathBuf,
    asset_type: AssetType,
    thumb: &ThumbnailToCreateWithPaths,
    storage: &Storage,
) -> Result<()> {
    let out_file_avif = storage.new_command_out_file(&thumb.avif_key).await?;
    let out_file_webp = storage.new_command_out_file(&thumb.webp_key).await?;
    let out_dimension = match thumb.ty {
        ThumbnailType::SmallSquare => processing::image::OutDimension::Crop {
            width: 200,
            height: 200,
        },
        ThumbnailType::LargeOrigAspect => {
            processing::image::OutDimension::KeepAspect { width: 400 }
        }
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let out_paths = vec![&out_file_avif, &out_file_webp];
    let thumbnail_params = ThumbnailParams {
        in_path: asset_path,
        outputs: out_paths,
        out_dimension,
    };
    let res = match asset_type {
        AssetType::Image => GenerateThumbnail::generate_thumbnail(thumbnail_params).await,
        AssetType::Video => GenerateThumbnail::generate_video_thumbnail(thumbnail_params).await,
    };
    tx.send(res).unwrap();
    let result = rx
        .in_current_span()
        .await
        .wrap_err("thumbnail task died or something")?;
    result?;
    out_file_webp.flush_to_storage().await?;
    out_file_avif.flush_to_storage().await?;
    Ok(())
}
