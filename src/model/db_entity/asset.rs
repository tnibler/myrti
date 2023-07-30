use chrono::NaiveDateTime;

use crate::model::{AssetId, AssetRootDirId};

use super::DbAssetType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAsset {
    pub id: AssetId,
    pub ty: DbAssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_path: String,
    pub hash: Option<Vec<u8>>,
    pub added_at: NaiveDateTime,
    pub file_created_at: Option<NaiveDateTime>,
    pub file_modified_at: Option<NaiveDateTime>,
    pub canonical_date: Option<NaiveDateTime>,
    pub thumb_path_small_square_jpg: Option<String>,
    pub thumb_path_small_square_webp: Option<String>,
    pub thumb_path_large_orig_jpg: Option<String>,
    pub thumb_path_large_orig_webp: Option<String>,
}
