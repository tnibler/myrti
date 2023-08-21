use super::DbAssetType;
use crate::model::{AssetId, AssetRootDirId};
use chrono::NaiveDateTime;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DbAsset {
    pub id: AssetId,
    pub ty: DbAssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_path: String,
    pub hash: Option<Vec<u8>>,
    pub added_at: NaiveDateTime,
    pub taken_date: Option<NaiveDateTime>,
    pub taken_date_local_fallback: Option<NaiveDateTime>,
    pub width: i64,
    pub height: i64,
    pub rotation_correction: Option<i32>,
    pub thumb_small_square_avif: Option<String>,
    pub thumb_small_square_webp: Option<String>,
    pub thumb_large_orig_avif: Option<String>,
    pub thumb_large_orig_webp: Option<String>,
    pub thumb_small_square_width: Option<i64>,
    pub thumb_small_square_height: Option<i64>,
    pub thumb_large_orig_width: Option<i64>,
    pub thumb_large_orig_height: Option<i64>,
    pub codec_name: Option<String>,
    pub bitrate: Option<i64>,
    pub resource_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAssetThumbnails {
    pub id: AssetId,
    pub ty: DbAssetType,
    pub thumb_small_square_avif: Option<String>,
    pub thumb_small_square_webp: Option<String>,
    pub thumb_large_orig_avif: Option<String>,
    pub thumb_large_orig_webp: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAssetPathOnDisk {
    pub id: AssetId,
    pub path_in_asset_root: String,
    pub asset_root_path: String,
}
