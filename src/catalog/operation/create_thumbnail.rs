use std::path::PathBuf;

use eyre::{Context, Report, Result};
use tracing::{instrument, Instrument};

use crate::{
    model::{
        repository::{self, pool::DbPool},
        AssetId, AssetType, ThumbnailType,
    },
    processing::{
        self,
        image::{generate_thumbnail, ThumbnailParams},
        video::create_snapshot,
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
    pub avif_path: PathBuf,
    pub webp_path: PathBuf,
}

#[instrument(skip(pool))]
pub async fn apply_create_thumbnail(pool: &DbPool, op: &CreateThumbnailWithPaths) -> Result<()> {
    for thumb in &op.thumbnails {
        // TODO unnecessary transaction
        let mut tx = pool
            .begin()
            .await
            .wrap_err("could not begin db transaction")?;
        match thumb.ty {
            ThumbnailType::SmallSquare => {
                repository::asset::set_asset_small_thumbnails(
                    tx.as_mut(),
                    op.asset_id,
                    &thumb.avif_path,
                    &thumb.webp_path,
                )
                .await
            }
            ThumbnailType::LargeOrigAspect => {
                repository::asset::set_asset_large_thumbnails(
                    tx.as_mut(),
                    op.asset_id,
                    &thumb.avif_path,
                    &thumb.webp_path,
                )
                .await
            }
        }
        .wrap_err("could not set asset thumbnails")?;
        tx.commit()
            .await
            .wrap_err("could not commit db transaction")?;
    }
    Ok(())
}

pub struct ThumbnailSideEffectResult {
    pub failed: Vec<(ThumbnailToCreateWithPaths, Report)>,
}

#[instrument(skip(pool))]
pub async fn perform_side_effects_create_thumbnail(
    pool: &DbPool,
    op: &CreateThumbnailWithPaths,
) -> Result<ThumbnailSideEffectResult> {
    let mut result = ThumbnailSideEffectResult {
        failed: Vec::default(),
    };
    if op.thumbnails.is_empty() {
        return Ok(result);
    }
    let in_path = repository::asset::get_asset_path_on_disk(pool, op.asset_id)
        .await?
        .path_on_disk();
    let asset = repository::asset::get_asset(pool, op.asset_id).await?;
    // TODO don't await sequentially. Not super bad because op.thumbnails is small but still
    match asset.base.ty {
        AssetType::Image => {
            for thumb in &op.thumbnails {
                match create_thumbnail_from_image(pool, in_path.clone(), thumb)
                    .in_current_span()
                    .await
                {
                    Ok(_) => (),
                    Err(err) => {
                        result.failed.push((thumb.clone(), err));
                    }
                }
            }
        }
        AssetType::Video => {
            let snapshot_dir = tempfile::tempdir().wrap_err("could not create temp directory")?;
            let snapshot_path = snapshot_dir.path().join(format!("{}.webp", op.asset_id.0));
            let snapshot_result = create_snapshot(&in_path, &snapshot_path)
                .in_current_span()
                .await
                .wrap_err("could not take video snapshot");
            match snapshot_result {
                Ok(_) => {
                    for thumb in &op.thumbnails {
                        match create_thumbnail_from_image(pool, snapshot_path.clone(), thumb)
                            .in_current_span()
                            .await
                        {
                            Ok(_) => {}
                            Err(err) => {
                                result.failed.push((thumb.clone(), err));
                            }
                        }
                    }
                }
                Err(err) => {
                    // a specific ThumbnailToCreateWithPaths is required to add a result to failed,
                    // so we just use the first here even though all of them failed in reality
                    result
                        .failed
                        .push((op.thumbnails.first().unwrap().clone(), err)); // unwrap because we
                                                                              // return early if op.thumbnails is emtpy
                }
            }
        }
    }
    Ok(result)
}

#[instrument(skip(pool))]
async fn create_thumbnail_from_image(
    pool: &DbPool,
    image_path: PathBuf,
    thumb: &ThumbnailToCreateWithPaths,
) -> Result<()> {
    let out_paths = vec![thumb.avif_path.clone(), thumb.webp_path.clone()];
    let out_dimension = match thumb.ty {
        ThumbnailType::SmallSquare => processing::image::OutDimension::Crop {
            width: 200,
            height: 200,
        },
        ThumbnailType::LargeOrigAspect => {
            processing::image::OutDimension::KeepAspect { width: 300 }
        }
    };
    let thumbnail_params = ThumbnailParams {
        in_path: image_path,
        out_paths,
        out_dimension,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    rayon::spawn(move || {
        let res = generate_thumbnail(thumbnail_params);
        tx.send(res).unwrap();
    });
    rx.in_current_span()
        .await
        .wrap_err("thumbnail task died or something")?
}
