use std::path::PathBuf;

use eyre::{Context, Report, Result};
use tracing::{instrument, Instrument};

use crate::model::AssetBase;
use crate::{
    catalog::{
        ResolvedExistingResourcePath, ResolvedNewResourcePath, ResolvedResourcePath, ResourcePath,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetId, AssetType, ResourceFileId, ThumbnailType,
    },
    processing::{
        self,
        image::{generate_thumbnail, ThumbnailParams},
        video::create_snapshot,
    },
};

use super::resource_path_on_disk;

#[derive(Debug, Clone)]
pub struct CreateThumbnail<P: ResourcePath> {
    pub asset_id: AssetId,
    pub thumbnails: Vec<ThumbnailToCreate<P>>,
}

#[derive(Debug, Clone)]
pub struct ThumbnailToCreate<P: ResourcePath> {
    pub ty: ThumbnailType,
    pub webp_file: P,
    pub avif_file: P,
}

#[instrument(skip(pool))]
pub async fn apply_create_thumbnail(
    pool: &DbPool,
    op: &CreateThumbnail<ResolvedResourcePath>,
) -> Result<()> {
    for created_thumb in op.thumbnails.iter() {
        let mut tx = pool
            .begin()
            .await
            .wrap_err("could not begin db transaction")?;
        let avif_resource_id: ResourceFileId = match &created_thumb.avif_file {
            ResolvedResourcePath::New(ResolvedNewResourcePath {
                data_dir_id,
                path_in_data_dir,
            }) => {
                repository::resource_file::insert_new_resource_file2(
                    tx.as_mut(),
                    *data_dir_id,
                    &path_in_data_dir,
                )
                .await?
            }
            ResolvedResourcePath::Existing(ResolvedExistingResourcePath {
                resource_dir_id,
                path_in_resource_dir,
            }) => todo!("thumbnails normally always create new resource files"),
        };
        let webp_resource_id: ResourceFileId = match &created_thumb.webp_file {
            ResolvedResourcePath::New(ResolvedNewResourcePath {
                data_dir_id,
                path_in_data_dir,
            }) => {
                repository::resource_file::insert_new_resource_file2(
                    tx.as_mut(),
                    *data_dir_id,
                    &path_in_data_dir,
                )
                .await?
            }
            ResolvedResourcePath::Existing(ResolvedExistingResourcePath {
                resource_dir_id,
                path_in_resource_dir,
            }) => todo!("thumbnails normally always create new resource files"),
        };
        repository::asset::set_asset_thumbnail(
            tx.as_mut(),
            op.asset_id,
            created_thumb.ty,
            avif_resource_id,
            webp_resource_id,
        )
        .await
        .wrap_err("could not set asset thumbnails")?;
        tx.commit()
            .await
            .wrap_err("could not commit db transaction")?;
    }
    Ok(())
}

pub struct ThumbnailSideEffectResult {
    failed: Vec<(ThumbnailToCreate<ResolvedResourcePath>, Report)>,
}

#[instrument(skip(pool))]
pub async fn perform_side_effects_create_thumbnail(
    pool: &DbPool,
    op: &CreateThumbnail<ResolvedResourcePath>,
) -> Result<ThumbnailSideEffectResult> {
    let in_path = repository::asset::get_asset_path_on_disk(pool, op.asset_id)
        .await?
        .path_on_disk();
    let asset = repository::asset::get_asset(pool, op.asset_id).await?;
    let mut result = ThumbnailSideEffectResult {
        failed: Vec::default(),
    };
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
            create_snapshot(&in_path, &snapshot_path)
                .in_current_span()
                .await
                .wrap_err("could not take video snapshot")?;
            for thumb in &op.thumbnails {
                match create_thumbnail_from_image(pool, snapshot_path.clone(), thumb)
                    .in_current_span()
                    .await
                {
                    Ok(_) => {
                        tracing::info!("done")
                    }
                    Err(err) => {
                        tracing::error!(%err);
                        result.failed.push((thumb.clone(), err));
                    }
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
    thumb: &ThumbnailToCreate<ResolvedResourcePath>,
) -> Result<()> {
    let out_paths = vec![
        resource_path_on_disk(pool, &thumb.avif_file).await?,
        resource_path_on_disk(pool, &thumb.webp_file).await?,
    ];
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
