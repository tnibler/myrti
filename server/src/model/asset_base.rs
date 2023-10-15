use camino::Utf8PathBuf as PathBuf;
use chrono::{DateTime, FixedOffset, Utc};
use eyre::{eyre, Context, Result};
use serde::Serialize;

use super::{
    repository::db_entity::{DbAsset, DbAssetType, DbTimestampInfo},
    util::{
        bool_to_int, datetime_from_db_repr, datetime_to_db_repr, hash_u64_to_vec8,
        hash_vec8_to_u64, path_to_string,
    },
    Asset, AssetId, AssetRootDirId, AssetSpe, AssetType, Image, Video,
};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Hash)]
pub struct Size {
    pub width: i64,
    pub height: i64,
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

impl TryFrom<&Asset> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: &Asset) -> Result<Self, Self::Error> {
        let file_path = path_to_string(&value.base.file_path)?;
        let (image, video) = match &value.sp {
            super::AssetSpe::Image(image) => (Some(image), None),
            super::AssetSpe::Video(video) => (None, Some(video)),
        };
        let timezone_offset: Option<String> = match value.base.timestamp_info {
            TimestampInfo::TzCertain(tz)
            | TimestampInfo::TzSetByUser(tz)
            | TimestampInfo::TzInferredLocation(tz)
            | TimestampInfo::TzGuessedLocal(tz) => Some(tz.to_string()),
            TimestampInfo::UtcCertain | TimestampInfo::NoTimestamp => None,
        };
        Ok(DbAsset {
            id: value.base.id,
            ty: value.base.ty.into(),
            root_dir_id: value.base.root_dir_id,
            file_type: value.base.file_type.clone(),
            file_path,
            is_hidden: if value.base.is_hidden { 1 } else { 0 },
            hash: value.base.hash.map(|h| hash_u64_to_vec8(h)),
            added_at: datetime_to_db_repr(&value.base.added_at),
            taken_date: datetime_to_db_repr(&value.base.taken_date),
            timezone_offset,
            timezone_info: (&value.base.timestamp_info).into(),
            width: value.base.size.width,
            height: value.base.size.height,
            rotation_correction: value.base.rotation_correction,
            gps_latitude: value.base.gps_coordinates.map(|c| c.lat),
            gps_longitude: value.base.gps_coordinates.map(|c| c.lon),
            thumb_small_square_avif: bool_to_int(value.base.thumb_small_square_avif),
            thumb_small_square_webp: bool_to_int(value.base.thumb_small_square_webp),
            thumb_large_orig_avif: bool_to_int(value.base.thumb_large_orig_avif),
            thumb_large_orig_webp: bool_to_int(value.base.thumb_large_orig_webp),
            thumb_small_square_width: value.base.thumb_small_square_size.as_ref().map(|s| s.width),
            thumb_small_square_height: value
                .base
                .thumb_small_square_size
                .as_ref()
                .map(|s| s.height),
            thumb_large_orig_width: value.base.thumb_large_orig_size.as_ref().map(|s| s.width),
            thumb_large_orig_height: value.base.thumb_large_orig_size.as_ref().map(|s| s.height),
            image_format_name: image.map(|i| i.image_format_name.clone()),
            video_codec_name: video.map(|v| v.video_codec_name.clone()),
            video_bitrate: video.map(|v| v.video_bitrate),
            audio_codec_name: video.map(|v| v.audio_codec_name.clone()).flatten(),
            has_dash: video.map(|v| if v.has_dash { 1 } else { 0 }),
        })
    }
}

impl TryFrom<Asset> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: Asset) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&DbAsset> for Asset {
    type Error = eyre::Report;

    fn try_from(value: &DbAsset) -> Result<Self, Self::Error> {
        let timezone_offset: Option<FixedOffset> = value
            .timezone_offset
            .as_ref()
            .map(|tz| tz.parse())
            .transpose()
            .wrap_err("could not parse timezone_offset")?;
        let timestamp_info = match (value.timezone_info, timezone_offset) {
            (DbTimestampInfo::UtcCertain, None) => TimestampInfo::UtcCertain,
            (DbTimestampInfo::TzCertain, Some(tz)) => TimestampInfo::TzCertain(tz),
            (DbTimestampInfo::TzSetByUser, Some(tz)) => TimestampInfo::TzSetByUser(tz),
            (DbTimestampInfo::TzInferredLocation, Some(tz)) => {
                TimestampInfo::TzInferredLocation(tz)
            }
            (DbTimestampInfo::TzGuessedLocal, Some(tz)) => TimestampInfo::TzGuessedLocal(tz),
            (DbTimestampInfo::NoTimestamp, _) => TimestampInfo::NoTimestamp,
            _ => {
                return Err(eyre!(
                    "Invalid combination of timezone_info and timezone_offset columns: {:?}, {:?}",
                    value.timezone_info,
                    timezone_offset
                ))
            }
        };
        let sp = match value.ty {
            DbAssetType::Image => AssetSpe::Image(Image {
                image_format_name: value
                    .image_format_name
                    .clone()
                    .ok_or(eyre!("image DbAsset must have image_format_name set"))?,
            }),
            DbAssetType::Video => AssetSpe::Video(Video {
                video_codec_name: value
                    .video_codec_name
                    .clone()
                    .ok_or(eyre!("video DbAsset must have video_codec_name set"))?,
                video_bitrate: value
                    .video_bitrate
                    .ok_or(eyre!("video DbAsset must have video_bitrate set"))?,
                audio_codec_name: value.audio_codec_name.clone(),
                has_dash: value
                    .has_dash
                    .map(|i| i != 0)
                    .ok_or(eyre!("Video asset can not have has_dash null"))?,
            }),
        };
        let hash: Option<u64> = value
            .hash
            .as_ref()
            .map(|a| hash_vec8_to_u64(&a))
            .transpose()?;
        let coords = match (value.gps_latitude, value.gps_longitude) {
            (Some(lat), Some(lon)) => Some(GpsCoordinates { lat, lon }),
            _ => None,
        };
        Ok(Asset {
            sp,
            base: AssetBase {
                id: value.id,
                ty: value.ty.into(),
                root_dir_id: value.root_dir_id,
                file_type: value.file_type.clone(),
                file_path: value.file_path.as_str().into(),
                is_hidden: value.is_hidden != 0,
                added_at: datetime_from_db_repr(value.added_at)?,
                hash,
                taken_date: datetime_from_db_repr(value.taken_date)?,
                timestamp_info,
                size: Size {
                    width: value.width,
                    height: value.height,
                },
                rotation_correction: value.rotation_correction,
                gps_coordinates: coords,
                thumb_small_square_avif: value.thumb_small_square_avif != 0,
                thumb_small_square_webp: value.thumb_small_square_webp != 0,
                thumb_large_orig_avif: value.thumb_large_orig_avif != 0,
                thumb_large_orig_webp: value.thumb_large_orig_webp != 0,
                thumb_small_square_size: value
                    .thumb_small_square_width
                    .zip(value.thumb_small_square_height)
                    .map(|(width, height)| Size { width, height }),
                thumb_large_orig_size: value
                    .thumb_large_orig_width
                    .zip(value.thumb_large_orig_height)
                    .map(|(width, height)| Size { width, height }),
            },
        })
    }
}

impl TryFrom<DbAsset> for Asset {
    type Error = eyre::Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
