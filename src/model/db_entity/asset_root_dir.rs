use crate::model::AssetRootDirId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAssetRootDir {
    pub id: AssetRootDirId,
    pub path: String,
}
