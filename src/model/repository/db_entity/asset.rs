use super::DbAssetType;
use crate::model::{AssetId, AssetRootDirId, TimestampInfo};

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DbAsset {
    pub id: AssetId,
    pub ty: DbAssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_type: String,
    pub file_path: String,
    pub hash: Option<Vec<u8>>,
    pub added_at: i64,
    pub taken_date: i64,
    pub timezone_offset: Option<String>,
    pub timezone_info: DbTimestampInfo,
    pub width: i64,
    pub height: i64,
    pub rotation_correction: Option<i32>,
    pub thumb_small_square_avif: i64,
    pub thumb_small_square_webp: i64,
    pub thumb_large_orig_avif: i64,
    pub thumb_large_orig_webp: i64,
    pub thumb_small_square_width: Option<i64>,
    pub thumb_small_square_height: Option<i64>,
    pub thumb_large_orig_width: Option<i64>,
    pub thumb_large_orig_height: Option<i64>,
    pub video_codec_name: Option<String>,
    pub video_bitrate: Option<i64>,
    pub audio_codec_name: Option<String>,
    pub has_dash: Option<i64>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, sqlx::Type)]
#[repr(i32)]
pub enum DbTimestampInfo {
    TzCertain = 1,
    UtcCertain = 2,
    TzSetByUser = 3,
    TzInferredLocation = 4,
    TzGuessedLocal = 5,
    NoTimestamp = 6,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAssetThumbnails {
    pub id: AssetId,
    pub ty: DbAssetType,
    pub thumb_small_square_avif: i64,
    pub thumb_small_square_webp: i64,
    pub thumb_large_orig_avif: i64,
    pub thumb_large_orig_webp: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbAssetPathOnDisk {
    pub id: AssetId,
    pub path_in_asset_root: String,
    pub asset_root_path: String,
}

impl From<&TimestampInfo> for DbTimestampInfo {
    fn from(value: &TimestampInfo) -> Self {
        match value {
            TimestampInfo::TzCertain(_) => Self::TzCertain,
            TimestampInfo::UtcCertain => Self::UtcCertain,
            TimestampInfo::TzSetByUser(_) => Self::TzSetByUser,
            TimestampInfo::TzInferredLocation(_) => Self::TzInferredLocation,
            TimestampInfo::TzGuessedLocal(_) => Self::TzGuessedLocal,
            TimestampInfo::NoTimestamp => Self::NoTimestamp,
        }
    }
}
