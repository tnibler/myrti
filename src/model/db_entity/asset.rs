use chrono::NaiveDateTime;

use crate::model::{AssetId, AssetRootDirId};

use super::DbAssetType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAsset {
    pub id: AssetId,
    pub ty: DbAssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_path: String,
    pub file_created_at: Option<NaiveDateTime>,
    pub file_modified_at: Option<NaiveDateTime>,
    pub thumb_path_jpg: Option<String>,
    pub thumb_path_webp: Option<String>,
}
