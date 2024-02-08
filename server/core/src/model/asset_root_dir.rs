use super::{repository::db_entity::DbAssetRootDir, AssetRootDirId};
use camino::Utf8PathBuf as PathBuf;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRootDir {
    pub id: AssetRootDirId,
    pub path: PathBuf,
}
