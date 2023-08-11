use std::path::PathBuf;

use eyre::{Context, Result};
use tracing::instrument;

use crate::model::{
    repository::{self, pool::DbPool},
    AssetThumbnails, ThumbnailType,
};

use super::{
    operation::create_thumbnail::{CreateThumbnail, ThumbnailToCreate},
    PathInResourceDir,
};

#[instrument(skip(pool))]
pub async fn thumbnails_to_create(
    pool: &DbPool,
) -> Result<Vec<CreateThumbnail<PathInResourceDir>>> {
    let limit: Option<i32> = None;
    let assets: Vec<AssetThumbnails> =
        repository::asset::get_assets_with_missing_thumbnail(pool, limit)
            .await
            .wrap_err("could not query for Assets with missing thumbnails")?;
    Ok(assets
        .into_iter()
        .map(|asset| CreateThumbnail {
            asset_id: asset.id,
            thumbnails: vec![
                ThumbnailToCreate {
                    ty: ThumbnailType::SmallSquare,
                    webp_file: PathBuf::from(format!("{}_sm.webp", asset.id.0)).into(),
                    avif_file: PathBuf::from(format!("{}_sm.avif", asset.id.0)).into(),
                },
                ThumbnailToCreate {
                    ty: ThumbnailType::LargeOrigAspect,
                    webp_file: PathBuf::from(format!("{}.webp", asset.id.0)).into(),
                    avif_file: PathBuf::from(format!("{}.avif", asset.id.0)).into(),
                },
            ],
        })
        .collect())
}
