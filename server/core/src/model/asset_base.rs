use camino::Utf8PathBuf as PathBuf;
use chrono::{DateTime, FixedOffset, Utc};
use serde::Serialize;

use super::{AssetId, AssetRootDirId, AssetType};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetBase {
    pub id: AssetId,
    pub ty: AssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_type: String,
    pub file_path: PathBuf,
    pub is_hidden: bool,
    pub added_at: DateTime<Utc>,
    pub taken_date: DateTime<Utc>,
    pub timestamp_info: TimestampInfo,
    pub size: Size,
    /// degrees clockwise
    pub rotation_correction: Option<i32>,
    pub gps_coordinates: Option<GpsCoordinates>,
    /// Seahash of the file, if already computed
    pub hash: Option<u64>,
    pub thumb_small_square_avif: bool,
    pub thumb_small_square_webp: bool,
    pub thumb_large_orig_avif: bool,
    pub thumb_large_orig_webp: bool,
    pub thumb_small_square_size: Option<Size>,
    pub thumb_large_orig_size: Option<Size>,
}

/// Origin and reliability of the timezone for an asset's original creation date
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimestampInfo {
    /// Asset metadata contained timezone information
    TzCertain(FixedOffset),
    /// Asset metadata contained UTC timestamp with no timezone information
    UtcCertain,
    /// Timezone was specified by the user
    TzSetByUser(FixedOffset),
    /// Timezone was inferred from location tags
    TzInferredLocation(FixedOffset),
    /// Asset metadata contained local date and time with no timezone info,
    /// so it was assumed to be the local timezone of this system
    TzGuessedLocal(FixedOffset),
    /// No timestamp in asset metadata, the timestamp is made up
    NoTimestamp,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Hash)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbnailType {
    SmallSquare,
    LargeOrigAspect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbnailFormat {
    Webp,
    Avif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpsCoordinates {
    /// multiplied by 10e8
    pub lat: i64,
    /// multiplied by 10e8
    pub lon: i64,
}

impl AssetBase {
    pub fn taken_date_local(&self) -> DateTime<FixedOffset> {
        match self.timestamp_info {
            TimestampInfo::NoTimestamp => self.taken_date.into(),
            TimestampInfo::UtcCertain => self.taken_date.into(),
            TimestampInfo::TzCertain(tz)
            | TimestampInfo::TzSetByUser(tz)
            | TimestampInfo::TzInferredLocation(tz)
            | TimestampInfo::TzGuessedLocal(tz) => self.taken_date.with_timezone(&tz),
        }
    }
}
