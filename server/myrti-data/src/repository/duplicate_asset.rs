use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use diesel::{dsl::delete, prelude::*};
use eyre::{Context, Result, eyre};

use crate::model::{AssetRootDirId, DuplicateFileId, FileId};

use super::schema::{self};
use crate::db::DbConn;

#[derive(Debug, Clone)]
pub struct NewDuplicateFile<'a> {
    pub existing_file_id: FileId,
    pub asset_root_dir_id: AssetRootDirId,
    pub path_in_asset_root: &'a Path,
}

pub fn insert_duplicate_file<'a>(
    conn: &mut DbConn,
    dupe: NewDuplicateFile<'a>,
) -> Result<DuplicateFileId> {
    use schema::DuplicateFile;
    let id = diesel::insert_into(DuplicateFile::table)
        .values((
            DuplicateFile::file_id.eq(dupe.existing_file_id.0),
            DuplicateFile::root_dir_id.eq(dupe.asset_root_dir_id.0),
            DuplicateFile::file_path.eq(dupe.path_in_asset_root.as_str()),
        ))
        .returning(DuplicateFile::dup_file_id)
        .get_result(conn)?;
    Ok(DuplicateFileId(id))
}

pub fn delete_duplicate_file(conn: &mut DbConn, id: DuplicateFileId) -> Result<()> {
    use schema::DuplicateFile;
    let n_affected = delete(DuplicateFile::table.find(id.0))
        .execute(conn)
        .wrap_err("error deleting from DuplicateFile")?;
    if n_affected != 1 {
        Err(eyre!("error deleting from DuplicateFile"))
    } else {
        Ok(())
    }
}

pub fn get_duplicate_path_on_disk(conn: &mut DbConn, id: DuplicateFileId) -> Result<PathBuf> {
    use schema::AssetRootDir;
    use schema::DuplicateFile;
    let (path, root_path): (String, String) = DuplicateFile::table
        .inner_join(AssetRootDir::table)
        .filter(DuplicateFile::dup_file_id.eq(id.0))
        .select((DuplicateFile::file_path, AssetRootDir::path))
        .first(conn)
        .wrap_err("error getting DuplicateFile path")?;
    Ok(PathBuf::from(root_path).join(&path))
}
