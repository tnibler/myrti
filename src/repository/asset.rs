use color_eyre::eyre;
use std::path::Path;


use crate::model::{
    entity::{DbAsset, DbAssetType},
    Asset, AssetBase, AssetId, FullAsset,
};

use super::pool::DbPool;

pub async fn insert_asset(pool: &DbPool, asset: FullAsset) -> eyre::Result<AssetId> {
    // let transaction = self.pool.begin().await.unwrap();
    let ty = match &asset.asset {
        Asset::Image {} => DbAssetType::Image,
        Asset::Video { dash_manifest_path: _ } => DbAssetType::Video,
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
    let result = sqlx::query!(
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
        ).execute(pool).await?;
    let rowid = result.last_insert_rowid();
    match &asset.asset {
        Asset::Image {} => {
            sqlx::query!(
                "
INSERT INTO ImageInfo (asset_id) VALUES
(?);
",
                rowid,
            )
            .execute(pool)
            .await?;
        }
        Asset::Video { dash_manifest_path } => {
            let dash_manifest_path = dash_manifest_path
                .as_ref()
                .map(|p| p.to_str().unwrap().to_string());
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
    };
    // transaction.commit().await?;
    Ok(AssetId(result.last_insert_rowid()))
}

pub async fn get_asset_with_path(pool: &DbPool, path: &Path) -> eyre::Result<Option<AssetBase>> {
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

pub async fn get_assets(pool: &DbPool) -> eyre::Result<Vec<AssetBase>> {
    sqlx::query_as!(
            DbAsset,
            r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets;
    "#)
        // TODO don't collect into vec before mapping
            .fetch_all(pool)
            .await?.into_iter().map(|r| Ok(r.into())).collect()
}
