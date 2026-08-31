use camino::Utf8Path as Path;
use diesel::prelude::*;
use eyre::Result;
use tracing::instrument;

use crate::model::{AssetRootDirId, DuplicateAssetId, FileId};

use super::schema::{self};
use crate::db::DbConn;

#[derive(Debug, Clone)]
pub struct NewDuplicateAsset<'a> {
    pub existing_file_id: FileId,
    pub asset_root_dir_id: AssetRootDirId,
    pub path_in_asset_root: &'a Path,
}

#[instrument(skip(conn))]
pub fn insert_duplicate_asset<'a>(
    conn: &mut DbConn,
    dupe: NewDuplicateAsset<'a>,
) -> Result<DuplicateAssetId> {
    use schema::DuplicateFile;
    let id = diesel::insert_into(DuplicateFile::table)
        .values((
            DuplicateFile::file_id.eq(dupe.existing_file_id.0),
            DuplicateFile::root_dir_id.eq(dupe.asset_root_dir_id.0),
            DuplicateFile::file_path.eq(dupe.path_in_asset_root.as_str()),
        ))
        .returning(DuplicateFile::dup_file_id)
        .get_result(conn)?;
    Ok(DuplicateAssetId(id))
}
