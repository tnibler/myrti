use camino::Utf8Path as Path;
use diesel::prelude::*;
use eyre::Result;
use tracing::instrument;

use crate::model::{AssetId, AssetRootDirId, DuplicateAssetId};

use super::db::DbConn;
use super::schema::{self};

#[derive(Debug, Clone)]
pub struct NewDuplicateAsset<'a> {
    pub existing_asset_id: AssetId,
    pub asset_root_dir_id: AssetRootDirId,
    pub path_in_asset_root: &'a Path,
}

#[instrument(skip(conn))]
pub fn insert_duplicate_asset<'a>(
    conn: &mut DbConn,
    dupe: NewDuplicateAsset<'a>,
) -> Result<DuplicateAssetId> {
    use schema::DuplicateAsset;
    let id = diesel::insert_into(DuplicateAsset::table)
        .values((
            DuplicateAsset::asset_id.eq(dupe.existing_asset_id.0),
            DuplicateAsset::root_dir_id.eq(dupe.asset_root_dir_id.0),
            DuplicateAsset::file_path.eq(dupe.path_in_asset_root.as_str()),
        ))
        .returning(DuplicateAsset::dup_asset_id)
        .get_result(conn)?;
    Ok(DuplicateAssetId(id))
}
