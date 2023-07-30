use crate::{
    model::{AssetAll, AssetBase, AssetId, AssetType},
    processing::{self, image::ThumbnailParams},
    repository::{self, pool::DbPool},
};
use eyre::{eyre, Context, Result};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{error, info, info_span, instrument};

pub async fn assets_without_thumbnails(pool: &DbPool) -> Result<Vec<AssetBase>> {
    repository::asset::get_assets_with_missing_thumbnail(pool, None).await
}

#[derive(Debug)]
pub struct ThumbnailResult {
    pub asset_id: AssetId,
    pub result: Result<()>,
}

#[instrument("Generate thumbnails", skip(pool, assets), fields(num_assets=assets.len()))]
pub async fn generate_thumbnails(
    assets: &Vec<AssetBase>,
    out_dir: &Path,
    pool: &DbPool,
) -> Result<Vec<ThumbnailResult>> {
    fs::create_dir_all(out_dir)
        .await
        .wrap_err("could not create thumbnail directory")?;
    let mut images = Vec::<AssetBase>::new();
    let mut videos = Vec::<AssetBase>::new();
    for asset in assets.into_iter() {
        match asset.ty {
            AssetType::Image => images.push(asset.clone()),
            AssetType::Video => videos.push(asset.clone()),
        }
    }
    info!(
        "Generating thumbnails for {} images, {} videos",
        images.len(),
        videos.len()
    );
    #[derive(Debug, Clone)]
    struct ThumbnailJob {
        pub id: AssetId,
        pub params: ThumbnailParams,
    }
    let mut image_thumbnail_params = Vec::<ThumbnailJob>::new();
    let mut already_failed = Vec::<ThumbnailResult>::new();
    for asset in images.into_iter() {
        let root_dir_result = repository::asset_root_dir::get_asset_root(&pool, asset.root_dir_id)
            .await
            .wrap_err(format!("could not get AssetRootDir for Asset {}", asset.id));
        let root_dir = match root_dir_result {
            Ok(r) => r,
            Err(e) => {
                already_failed.push(ThumbnailResult {
                    asset_id: asset.id,
                    result: Err(e),
                });
                continue;
            }
        };
        let in_path = root_dir.path.join(&asset.file_path);
        let out_path_jpg = out_dir.join(format!("{}.jpg", asset.id.0));
        let out_path_webp = out_dir.join(format!("{}.webp", asset.id.0));
        let out_paths = vec![out_path_jpg.clone(), out_path_webp.clone()];
        image_thumbnail_params.push(ThumbnailJob {
            id: asset.id,
            params: ThumbnailParams {
                in_path,
                out_paths,
                width: 300,
                height: 300,
            },
        });
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<Vec<ThumbnailResult>>();
    rayon::spawn(move || {
        // TODO panicking in here panics the application instead of only the job
        let result = image_thumbnail_params
            .into_par_iter()
            .map(|job| ThumbnailResult {
                asset_id: job.id,
                result: processing::image::generate_thumbnail(job.params)
                    .wrap_err("An error occurred while generating the image thumbnail"),
            })
            .collect::<Vec<ThumbnailResult>>();
        tx.send(result).unwrap();
    });
    let mut results = rx.await.unwrap();
    results.append(&mut already_failed);
    return Ok(results);
}
