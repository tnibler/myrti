use super::{repository::db_entity::DbAssetRootDir, AssetRootDirId};
use camino::Utf8PathBuf as PathBuf;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRootDir {
    pub id: AssetRootDirId,
    pub path: PathBuf,
}

impl TryFrom<&DbAssetRootDir> for AssetRootDir {
    type Error = eyre::Report;

    fn try_from(value: &DbAssetRootDir) -> Result<Self, Self::Error> {
        Ok(AssetRootDir {
            id: value.id,
            path: PathBuf::from(&value.path),
        })
    }
}

impl TryFrom<DbAssetRootDir> for AssetRootDir {
    type Error = eyre::Report;

    fn try_from(value: DbAssetRootDir) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&AssetRootDir> for DbAssetRootDir {
    type Error = eyre::Report;

    fn try_from(value: &AssetRootDir) -> Result<Self, Self::Error> {
        let path = value.path.to_string();
        Ok(DbAssetRootDir { id: value.id, path })
    }
}

impl TryFrom<AssetRootDir> for DbAssetRootDir {
    type Error = eyre::Report;

    fn try_from(value: AssetRootDir) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
