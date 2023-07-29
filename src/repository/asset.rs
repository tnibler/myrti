use color_eyre::eyre;
use eyre::Result;
use std::path::Path;

use crate::model::{
    entity::{DbAsset, DbAssetType},
    Asset, AssetBase, AssetId, FullAsset,
};

use super::pool::DbPool;

pub async fn insert_asset(pool: &DbPool, asset: FullAsset) -> Result<AssetId> {
    insert_or_update_asset(pool, asset, false).await
}

pub async fn update_asset(pool: &DbPool, asset: FullAsset) -> Result<AssetId> {
    insert_or_update_asset(pool, asset, true).await
}

async fn insert_or_update_asset(pool: &DbPool, asset: FullAsset, update: bool) -> Result<AssetId> {
    // let transaction = self.pool.begin().await.unwrap();
    let ty = match &asset.asset {
        Asset::Image {} => DbAssetType::Image,
        Asset::Video {
            dash_manifest_path: _,
        } => DbAssetType::Video,
    };
    let file_path = asset
        .base
        .file_path
        .canonicalize()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let thumb_path_jpg = asset
        .base
        .thumb_path_jpg
        .map(|p| p.canonicalize().unwrap().to_str().unwrap().to_string());
    let thumb_path_webp = asset
        .base
        .thumb_path_webp
        .map(|p| p.canonicalize().unwrap().to_str().unwrap().to_string());

    let result = if update {
        sqlx::query!(
"
UPDATE Assets SET ty=?, root_dir_id=?, file_path=?, file_created_at=?, file_modified_at=?, thumb_path_jpg=?, thumb_path_webp=? 
WHERE id=?;
",
            ty,
            asset.base.root_dir_id,
            file_path,
            asset.base.file_created_at,
            asset.base.file_modified_at,
            thumb_path_jpg,
            thumb_path_webp,
            asset.base.id.0
        ).execute(pool).await?
    } else {
        sqlx::query!(
"
INSERT INTO Assets (id, ty, root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp) VALUES
(null, ?, ?, ?, ?, ?, ?, ?);
",
            ty,
            asset.base.root_dir_id,
            file_path,
            asset.base.file_created_at,
            asset.base.file_modified_at,
            thumb_path_jpg,
            thumb_path_webp,
        ).execute(pool).await?
    };
    let rowid = result.last_insert_rowid();
    match &asset.asset {
        Asset::Image {} => {
            if update {
                // sqlx::query!("UPDATE ImageInfo SET a=b WHERE asset_id=?",
                // asset.base.id.0).await?;
            } else {
                sqlx::query!(
                    "INSERT INTO ImageInfo (asset_id) VALUES(?);",
                    rowid,
                )
                    .execute(pool)
                .await?;
            }
        }
        Asset::Video { dash_manifest_path } => {
            let dash_manifest_path = dash_manifest_path
                .as_ref()
                .map(|p| p.to_str().unwrap().to_string());
            if update {
                sqlx::query!(
                    "
UPDATE VideoInfo SET dash_manifest_path=? 
WHERE asset_id=?;
",
                    dash_manifest_path, 
                    asset.base.id.0
                )
                .execute(pool)
                .await?;
            } else {
                sqlx::query!(
                    "
INSERT INTO VideoInfo (asset_id, dash_manifest_path) VALUES
(?, ?);
",
                    rowid,
                    dash_manifest_path
                )
                .execute(pool)
                .await?;
            }
        }
    };
    if update {
        Ok(asset.base.id)
    } else {
        Ok(AssetId(result.last_insert_rowid()))
    }
}


pub async fn get_asset_with_path(pool: &DbPool, path: &Path) -> Result<Option<AssetBase>> {
    let path = path.to_str().unwrap();
    let db_asset = sqlx::query_as!(
            DbAsset,
            r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets WHERE file_path = ?;
    "#,
            path
        )
        .fetch_optional(pool)
        .await?;
    Ok(db_asset.map(|db_asset| db_asset.into()))
}

pub async fn get_assets(pool: &DbPool) -> Result<Vec<AssetBase>> {
    sqlx::query_as!(
            DbAsset,
            r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets;
    "#)
        // TODO don't collect into vec before mapping
            .fetch_all(pool)
            .await?.into_iter().map(|r| Ok(r.into())).collect()
}

pub async fn get_assets_with_missing_thumbnail(
    pool: &DbPool,
    limit: Option<i32>,
) -> Result<Vec<AssetBase>> {
    if let Some(limit) = limit {
        sqlx::query_as!(DbAsset,
    r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets
WHERE thumb_path_jpg IS NULL OR thumb_path_webp IS NULL
LIMIT ?;
    "#, limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(DbAsset,
    r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets
WHERE thumb_path_jpg IS NULL OR thumb_path_webp IS NULL;
    "#)
        .fetch_all(pool)
        .await?
    } 
        .into_iter().map(|r| Ok(r.into())).collect()
}
