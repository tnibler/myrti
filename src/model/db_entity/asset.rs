use super::DbAssetType;
use crate::model::{AssetId, AssetRootDirId, ResourceFileId};
use chrono::NaiveDateTime;

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
    pub thumb_small_square_jpg: Option<ResourceFileId>,
    pub thumb_small_square_webp: Option<ResourceFileId>,
    pub thumb_large_orig_jpg: Option<ResourceFileId>,
    pub thumb_large_orig_webp: Option<ResourceFileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAssetThumbnails {
    pub id: AssetId,
    pub ty: DbAssetType,
    pub thumb_small_square_jpg: Option<ResourceFileId>,
    pub thumb_small_square_webp: Option<ResourceFileId>,
    pub thumb_large_orig_jpg: Option<ResourceFileId>,
    pub thumb_large_orig_webp: Option<ResourceFileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAssetPathOnDisk {
    pub id: AssetId,
    pub path_in_asset_root: String,
    pub asset_root_path: String,
}
