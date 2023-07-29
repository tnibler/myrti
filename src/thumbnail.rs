use crate::{
    model::{AssetAll, AssetBase, AssetType},
    processing::{self, image::ThumbnailParams},
    repository::{self, pool::DbPool},
};
use eyre::{eyre, Context, Result};
use rayon::prelude::*;
use std::{
    ffi::{c_int, CString},
    path::{Path, PathBuf},
};
use tokio::fs;
use tracing::{error, info, info_span, instrument};

pub async fn assets_without_thumbnails(pool: &DbPool) -> Result<Vec<AssetBase>> {
    repository::asset::get_assets_with_missing_thumbnail(pool, None).await
}

#[instrument("Generate thumbnails", skip(pool, assets), fields(num_assets=assets.len()))]
pub async fn generate_thumbnails(
    assets: &Vec<AssetBase>,
    out_dir: &Path,
    pool: &DbPool,
) -> Result<()> {
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
    let mut image_thumbnail_params = Vec::<ThumbnailParams>::new();
    for asset in images.into_iter() {
        let root_dir = repository::asset_root_dir::get_asset_root(&pool, asset.root_dir_id)
            .await
            .wrap_err(format!("could not get AssetRootDir for Asset {}", asset.id))?;
        let in_path = root_dir.path.join(&asset.file_path);
        let out_path_jpg = out_dir.join(format!("{}.jpg", asset.id.0));
        let out_path_webp = out_dir.join(format!("{}.webp", asset.id.0));
        let out_paths = vec![out_path_jpg.clone(), out_path_webp.clone()];
        image_thumbnail_params.push(ThumbnailParams {
            in_path,
            out_paths,
            width: 300,
            height: 300,
        });
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<Vec<()>>>();
    rayon::spawn(move || {
        let result = image_thumbnail_params
            .into_par_iter()
            .map(|params| processing::image::generate_thumbnail(params))
            .collect::<Result<Vec<_>>>();
        tx.send(result).unwrap();
    });
    let result = rx.await.unwrap();
    if let Err(e) = result {
        error!(
            error = e.to_string(),
            "An error occurred while generating image thumbnails"
        );
        return Err(e);
    }
    Ok(())
}
