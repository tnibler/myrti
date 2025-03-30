use camino::Utf8Path as Path;
use diesel::prelude::*;
use eyre::Result;
use tracing::instrument;

use crate::model::{AssetRootDir, AssetRootDirId};

use super::db::DbConn;
use super::db_entity::DbAssetRootDir;
use super::schema;

#[instrument(skip(conn), level = "trace")]
pub fn get_asset_root(conn: &mut DbConn, id: AssetRootDirId) -> Result<AssetRootDir> {
    use schema::AssetRootDir;
    let db_ard: DbAssetRootDir = AssetRootDir::table.find(id.0).first(conn)?;
    db_ard.try_into()
}

#[instrument(skip(conn), level = "trace")]
pub fn get_asset_roots(conn: &mut DbConn) -> Result<Vec<AssetRootDir>> {
    use schema::AssetRootDir;
    let db_ards: Vec<DbAssetRootDir> = AssetRootDir::table.load(conn)?;
    db_ards
        .into_iter()
        .map(|db_ard| db_ard.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn), level = "trace")]
pub fn insert_asset_root(
    conn: &mut DbConn,
    asset_root_dir: &AssetRootDir,
) -> Result<AssetRootDirId> {
    use schema::AssetRootDir;
    let id = diesel::insert_into(AssetRootDir::table)
        .values(AssetRootDir::path.eq(&asset_root_dir.path.as_str()))
        .returning(AssetRootDir::asset_root_dir_id)
        .get_result(conn)?;
    Ok(AssetRootDirId(id))
}

#[instrument(skip(conn), level = "trace")]
pub fn get_asset_root_with_path(conn: &mut DbConn, path: &Path) -> Result<Option<AssetRootDir>> {
    use schema::AssetRootDir;
    let db_ard: Option<DbAssetRootDir> = AssetRootDir::table
        .filter(AssetRootDir::path.eq(path.as_str()))
        .first(conn)
        .optional()?;
    db_ard.map(|db_ard| db_ard.try_into()).transpose()
}
