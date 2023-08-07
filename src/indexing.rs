use std::path::Path;

use crate::{
    model::*,
    processing,
    repository::{self, pool::DbPool},
};
use chrono::Utc;
use color_eyre::eyre::Result;
use eyre::Context;
use tokio::fs;
use tracing::{debug, error, instrument};
use walkdir::WalkDir;

pub async fn index_asset_root(asset_root: &AssetRootDir, pool: &DbPool) -> Result<Vec<AssetId>> {
    let mut new_asset_ids: Vec<AssetId> = vec![];
    // FIXME if a datadir is subdir of assetroot it should obviously not be indexed
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

#[instrument(name = "Index file", skip(pool))]
async fn index_file(
    path: &Path,
    asset_root: &AssetRootDir,
    pool: &DbPool,
) -> Result<Option<AssetId>> {
    let path_in_asset_root = path
        .strip_prefix(&asset_root.path)
        .wrap_err("file to index is not in provided asset root")?;
    match repository::asset::get_asset_with_path(pool, path_in_asset_root).await? {
        Some(_) => Ok(None),
        None => {
            if let Some(extension) = path.extension().map(|s| s.to_string_lossy().to_string()) {
                let (ty, full): (AssetType, AssetAll) = match extension.as_str() {
                    "mp4" => {
                        let video_info = AssetAll::Video(Video {
                            dash_resource_dir: None,
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
                let size: Size = match ty {
                    AssetType::Image => {
                        let p = path.to_owned();
                        let s = tokio::task::spawn_blocking(move || {
                            processing::image::get_image_size(&p)
                                .wrap_err("could not read image size")
                        })
                        .await??;
                        Size {
                            width: s.width.into(),
                            height: s.height.into(),
                        }
                    }
                    AssetType::Video => {
                        let probe = processing::video::probe_video(path)
                            .await
                            .wrap_err("could not read video info using ffprobe")?;
                        Size {
                            width: probe.width.into(),
                            height: probe.height.into(),
                        }
                    }
                };
                let asset_base = AssetBase {
                    id: AssetId(0),
                    ty,
                    root_dir_id: asset_root.id,
                    file_path: path.strip_prefix(&asset_root.path)?.to_owned(),
                    file_created_at: metadata.created().ok().map(|t| t.into()),
                    file_modified_at: metadata.modified().ok().map(|t| t.into()),
                    added_at: Utc::now(),
                    canonical_date: None,
                    size,
                    hash: None,
                    thumb_small_square_jpg: None,
                    thumb_small_square_webp: None,
                    thumb_large_orig_jpg: None,
                    thumb_large_orig_webp: None,
                    thumb_small_square_size: None,
                    thumb_large_orig_size: None,
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
