use std::path::Path;

use super::db_entity::DbAssetRootDir;
use super::pool::DbPool;
use crate::model::{util::path_to_string, AssetRootDir, AssetRootDirId};
use eyre::{eyre, Context, Result};

pub async fn get_asset_root(pool: &DbPool, id: AssetRootDirId) -> Result<AssetRootDir> {
    match sqlx::query_as!(DbAssetRootDir, "SELECT * FROM AssetRootDir WHERE id=?", id)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(r)) => r.try_into(),
        Ok(None) => Err(eyre!("no AssetRootDir with id {}", id)),
        Err(e) => Err(e).wrap_err("failed to query table AssetRootDirs"),
    }
}

pub async fn get_asset_roots(pool: &DbPool) -> Result<Vec<AssetRootDir>> {
    sqlx::query_as!(DbAssetRootDir, "SELECT * FROM AssetRootDir;")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|ard| ard.try_into())
        .collect::<Result<Vec<_>>>()
        .wrap_err("failed to query table AssetRootDirs")
}

pub async fn insert_asset_root(
    pool: &DbPool,
    asset_root_dir: AssetRootDir,
) -> Result<AssetRootDirId> {
    let path = path_to_string(&asset_root_dir.path)?;
    sqlx::query!(
        "INSERT INTO AssetRootDir (id, path) VALUES (null, ?);",
        path
    )
    .execute(pool)
    .await
    .map(|query_result| AssetRootDirId(query_result.last_insert_rowid()))
    .wrap_err("failed to insert into table AssetRootDirs")
}

pub async fn get_asset_root_with_path(pool: &DbPool, path: &Path) -> Result<Option<AssetRootDir>> {
    // TODO handle errors or better dont canonicalize
    let path = path.canonicalize().unwrap().to_str().unwrap().to_string();
    sqlx::query_as!(
        DbAssetRootDir,
        "SELECT * FROM AssetRootDir WHERE path=?",
        path
    )
    .fetch_optional(pool)
    .await
    .wrap_err("failed to query table AssetRootDirs")?
    .map(|v| v.try_into())
    .transpose()
}
