use std::path::PathBuf;

use eyre::{Context, Result};

use crate::model::{
    repository::{self, pool::DbPool},
    AssetThumbnails, ThumbnailType,
};

use super::operation::{PathInResourceDir, ThumbnailToCreate};

pub async fn thumbnails_to_create(
    pool: &DbPool,
) -> Result<Vec<ThumbnailToCreate<PathInResourceDir>>> {
    let limit: Option<i32> = None;
    let assets: Vec<AssetThumbnails> =
        repository::asset::get_assets_with_missing_thumbnail(pool, limit)
            .await
            .wrap_err("could not query for Assets with missing thumbnails")?;
    Ok(assets
        .into_iter()
        .map(|asset| {
            [
                ThumbnailToCreate {
                    ty: ThumbnailType::SmallSquare,
                    webp_file: PathBuf::from(format!("thumbnails/{}_sm.webp", asset.id.0)).into(),
                    avif_file: PathBuf::from(format!("thumbnails/{}_sm.avif", asset.id.0)).into(),
                },
                ThumbnailToCreate {
                    ty: ThumbnailType::LargeOrigAspect,
                    webp_file: PathBuf::from(format!("thumbnails/{}.webp", asset.id.0)).into(),
                    avif_file: PathBuf::from(format!("thumbnails/{}.avif", asset.id.0)).into(),
                },
            ]
            .into_iter()
        })
        .flatten()
        .collect())
}
