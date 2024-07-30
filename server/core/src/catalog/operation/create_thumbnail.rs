use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Report, Result};
use futures::{stream::FuturesUnordered, TryStreamExt};
use tracing::instrument;

use crate::{
    core::storage::{CommandOutputFile, Storage, StorageCommandOutput, StorageProvider},
    interact,
    model::{
        repository::{
            self,
            db::{DbPool, PooledDbConn},
        },
        AssetId, AssetThumbnail, AssetThumbnailId, AssetType, Size, ThumbnailFormat, ThumbnailType,
    },
    processing::{
        self,
        commands::GenerateThumbnail,
        image::thumbnail::{GenerateThumbnailTrait, ThumbnailParams, ThumbnailResult},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetThumbnail {
    pub asset_id: AssetId,
    pub thumbnails: Vec<ThumbnailToCreate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreate {
    pub ty: ThumbnailType,
    pub formats: Vec<ThumbnailFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateThumbnailWithPaths {
    pub asset_id: AssetId,
    pub thumbnails: Vec<ThumbnailToCreateWithPaths>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreateWithPaths {
    pub ty: ThumbnailType,
    pub file_keys: Vec<(ThumbnailFormat, String)>,
}

#[instrument(skip(conn), level = "debug")]
pub async fn apply_create_thumbnail(
    conn: &mut PooledDbConn,
    result: ThumbnailSideEffectSuccess,
) -> Result<()> {
    interact!(conn, move |conn| {
        repository::asset::insert_asset_thumbnail(
            conn,
            AssetThumbnail {
                id: AssetThumbnailId(0),
                asset_id: result.asset_id,
                ty: result.ty,
                size: result.actual_size,
                format: result.format,
            },
        )?;
        Ok(())
    })
    .await??;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ThumbnailSideEffectSuccess {
    pub asset_id: AssetId,
    pub ty: ThumbnailType,
    pub format: ThumbnailFormat,
    pub actual_size: Size,
}

pub struct ThumbnailSideEffectResult {
    pub succeeded: Vec<ThumbnailSideEffectSuccess>,
    pub failed: Vec<(ThumbnailToCreateWithPaths, Report)>,
}

#[instrument(skip(pool, storage), level = "debug")]
pub async fn perform_side_effects_create_thumbnail(
    storage: &Storage,
    pool: DbPool,
    op: CreateThumbnailWithPaths,
) -> Result<ThumbnailSideEffectResult> {
    let mut result = ThumbnailSideEffectResult {
        succeeded: Vec::default(),
        failed: Vec::default(),
    };
    if op.thumbnails.is_empty() {
        return Ok(result);
    }
    let conn = pool.get().await?;
    let (in_path, asset) = interact!(conn, move |conn| {
        let in_path = repository::asset::get_asset_path_on_disk(conn, op.asset_id)?.path_on_disk();
        let asset = repository::asset::get_asset(conn, op.asset_id)?;
        Ok::<_, eyre::Report>((in_path, asset))
    })
    .await??;
    // TODO don't await sequentially. Not super bad because op.thumbnails is small but still
    for thumb in op.thumbnails {
        match create_thumbnail(in_path.clone(), asset.base.ty, &thumb, storage).await {
            Ok(res) => {
                for (format, _file_key) in thumb.file_keys {
                    result.succeeded.push(ThumbnailSideEffectSuccess {
                        asset_id: op.asset_id,
                        ty: thumb.ty,
                        format,
                        actual_size: res.actual_size,
                    });
                }
            }
            Err(err) => {
                result.failed.push((thumb.clone(), err));
            }
        }
    }
    Ok(result)
}

#[instrument(skip(storage))]
async fn create_thumbnail(
    asset_path: PathBuf,
    asset_type: AssetType,
    thumb: &ThumbnailToCreateWithPaths,
    storage: &Storage,
) -> Result<ThumbnailResult> {
    let out_files: Vec<CommandOutputFile> = thumb
        .file_keys
        .iter()
        .map(|(_format, key)| storage.new_command_out_file(key))
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await
        .wrap_err("error creating asset thumbnail output files")?;
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
    let thumbnail_params = ThumbnailParams {
        in_path: asset_path,
        outputs: out_files.iter().collect(),
        out_dimension,
    };
    let res = match asset_type {
        AssetType::Image => GenerateThumbnail::generate_thumbnail(thumbnail_params).await,
        AssetType::Video => GenerateThumbnail::generate_video_thumbnail(thumbnail_params).await,
    };
    tx.send(res).unwrap();
    let result = rx.await.wrap_err("thumbnail task died or something")??;
    for out_file in out_files {
        out_file.flush_to_storage().await?;
    }
    Ok(result)
}
