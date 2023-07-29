use super::{db_entity::DbAssetRootDir, AssetRootDirId};
use eyre::eyre;
use serde::Serialize;
use std::path::PathBuf;

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
        let path = value
            .path
            .to_str()
            .ok_or_else(|| eyre!("non unicode file path not supported"))?
            .to_string();
        Ok(DbAssetRootDir { id: value.id, path })
    }
}

impl TryFrom<AssetRootDir> for DbAssetRootDir {
    type Error = eyre::Report;

    fn try_from(value: AssetRootDir) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
