use std::path::Path;

use crate::{
    model::*,
    repository::{self, pool::DbPool},
};
use chrono::{DateTime, Utc};
use color_eyre::eyre::Result;
use tokio::fs;
use tracing::{debug, error};
use walkdir::WalkDir;

pub async fn index_asset_root(asset_root: &AssetRootDir, pool: &DbPool) -> Result<Vec<AssetId>> {
    let mut new_asset_ids: Vec<AssetId> = vec![];
    for entry in WalkDir::new(asset_root.path.as_path()).follow_links(true) {
        match entry {
            Ok(e) => {
                if e.file_type().is_file() {
                    if let Some(id) = index_file(e.path(), asset_root, pool).await? {
                        new_asset_ids.push(id);
                    }
                }
            }
            Err(e) => {
                if let Some(path) = e.path() {
                    error!("Could not index file or directory {}", path.display());
                } else {
                    error!(
                        "Error during indexing of asset root dir {}",
                        asset_root.path.as_path().display()
                    )
                }
            }
        }
    }
    Ok(new_asset_ids)
}

async fn index_file(
    path: &Path,
    asset_root: &AssetRootDir,
    pool: &DbPool,
) -> Result<Option<AssetId>> {
    assert!(path.starts_with(&asset_root.path));
    match repository::asset::get_asset_with_path(pool, path).await? {
        Some(_) => Ok(None),
        None => {
            if let Some(extension) = path.extension().map(|s| s.to_string_lossy().to_string()) {
                let (ty, full): (AssetType, AssetAll) = match extension.as_str() {
                    "mp4" => {
                        let video_info = AssetAll::Video(Video {
                            dash_manifest_path: None,
                        });
                        (AssetType::Video, video_info)
                    }
                    "jpg" => {
                        let image_info = AssetAll::Image(Image {});
                        (AssetType::Image, image_info)
                    }
                    _ => {
                        debug!("Ignoring file {}", path.display());
                        return Ok(None);
                    }
                };
                let metadata = fs::metadata(path).await?;
                let asset_base = AssetBase {
                    id: AssetId(0),
                    ty,
                    root_dir_id: asset_root.id,
                    file_path: path.to_owned(),
                    hash: None,
                    added_at: Utc::now(),
                    file_created_at: metadata.created().ok().map(|t| t.into()),
                    file_modified_at: metadata.modified().ok().map(|t| t.into()),
                    canonical_date: None,
                    thumb_path_small_square_jpg: None,
                    thumb_path_small_square_webp: None,
                    thumb_path_large_orig_jpg: None,
                    thumb_path_large_orig_webp: None,
                };
                let full_asset = FullAsset {
                    base: asset_base,
                    asset: full,
                };
                let id = repository::asset::insert_asset(pool, full_asset).await?;
                Ok(Some(id))
            } else {
                debug!("Ignoring file {}", path.display());
                return Ok(None);
            }
        }
    }
}
