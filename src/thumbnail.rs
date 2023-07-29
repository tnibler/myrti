use std::{
    ffi::{c_int, CString},
    os::unix::prelude::OsStrExt,
    path::PathBuf,
};

use crate::{
    model::{AssetAll, AssetBase, AssetType},
    repository::{self, pool::DbPool},
};
use eyre::{eyre, Result};
use tokio::fs;

include!(concat!(env!("OUT_DIR"), "/vips_wrapper_bindings.rs"));

pub async fn assets_without_thumbnails(pool: &DbPool) -> Result<Vec<AssetBase>> {
    repository::asset::get_assets_with_missing_thumbnail(pool, None).await
}

pub async fn generate_thumbnails(assets: &Vec<AssetBase>, pool: &DbPool) -> Result<()> {
    println!("calling vips!");
    unsafe {
        let i = init();
    }
    let out_dir = PathBuf::from("thumbnails");
    fs::create_dir_all(&out_dir).await.unwrap();
    for asset in assets.iter() {
        match asset.ty {
            AssetType::Image => {
                // TODO maybe some thumbnails are actually already set here,
                // don't always generate all of them
                let root_dir = repository::asset_root_dir::get_asset_root(&pool, asset.root_dir_id)
                    .await
                    .unwrap();
                let p = root_dir.path.join(&asset.file_path);
                let c_path = CString::new(p.as_os_str().as_bytes()).unwrap();
                let out_path_jpg = out_dir.join(format!("{}.jpg", asset.id.0));
                let out_path_webp = out_dir.join(format!("{}.webp", asset.id.0));
                let out_paths = vec![out_path_jpg.clone(), out_path_webp.clone()];
                tokio::task::spawn_blocking(move || unsafe {
                    let c_out_paths = out_paths
                        .iter()
                        .map(|path| CString::new(path.as_os_str().as_bytes()).unwrap())
                        .collect::<Vec<_>>();
                    let c_out_path_ptrs =
                        c_out_paths.iter().map(|p| p.as_ptr()).collect::<Vec<_>>();
                    let params = ThumbnailParams {
                        width: 300,
                        height: 300,
                        in_path: c_path.as_ptr(),
                        out_paths: c_out_path_ptrs.as_ptr(),
                        num_out_paths: c_out_path_ptrs.len() as i32,
                    };
                    let i = thumbnail(params);
                    if i != 0 {
                        return Err(eyre!("Error in vips_wrapper"));
                    }
                    Ok(())
                })
                .await
                .unwrap()?;
                let mut asset_with_thumbs = asset.clone();
                asset_with_thumbs.thumb_path_jpg = Some(out_path_jpg);
                asset_with_thumbs.thumb_path_webp = Some(out_path_webp);
                repository::asset::update_asset_base(
                    &mut pool.acquire().await.unwrap(),
                    &asset_with_thumbs,
                )
                .await?;
            }
            AssetType::Video => {}
        }
    }
    Ok(())
}
