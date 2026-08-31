use camino::Utf8PathBuf as PathBuf;
use diesel::{Queryable, Selectable};

use crate::model::{AssetRootDir, AssetRootDirId};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::AssetRootDir)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAssetRootDir {
    pub asset_root_dir_id: i64,
    pub path: String,
}

impl TryFrom<DbAssetRootDir> for AssetRootDir {
    type Error = eyre::Report;

    fn try_from(value: DbAssetRootDir) -> Result<Self, Self::Error> {
        Ok(AssetRootDir {
            id: AssetRootDirId(value.asset_root_dir_id),
            path: PathBuf::from(&value.path),
        })
    }
}
