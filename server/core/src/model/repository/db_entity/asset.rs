use std::borrow::Cow;

use camino::Utf8PathBuf as PathBuf;
use chrono::FixedOffset;
use diesel::{prelude::Insertable, Queryable, QueryableByName, Selectable};
use eyre::{eyre, Context, Result};

use crate::model::{
    util::{datetime_from_db_repr, hash_vec8_to_u64},
    Asset, AssetBase, AssetId, AssetPathOnDisk, AssetRootDirId, AssetSpe, AssetType,
    GpsCoordinates, Image, Size, TimestampInfo, Video,
};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, QueryableByName, Selectable)]
#[diesel(table_name = super::super::schema::Asset)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAsset {
    pub asset_id: i64,
    pub ty: i32,
    pub root_dir_id: i64,
    pub file_type: String,
    pub file_path: String,
    pub hash: Option<Vec<u8>>,
    pub is_hidden: i32,
    pub added_at: i64,
    pub taken_date: i64,
    pub timezone_offset: Option<String>,
    pub timezone_info: i32,
    pub width: i32,
    pub height: i32,
    pub rotation_correction: Option<i32>,
    pub gps_latitude: Option<i64>,
    pub gps_longitude: Option<i64>,
    pub image_format_name: Option<String>,
    pub video_codec_name: Option<String>,
    pub video_bitrate: Option<i64>,
    pub audio_codec_name: Option<String>,
    pub has_dash: Option<i32>,
}

impl TryFrom<DbAsset> for Asset {
    type Error = eyre::Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        let ty = from_db_asset_ty(value.ty)?;
        let timestamp_info =
            from_db_timezone_info(value.timezone_info, value.timezone_offset.as_deref())?;
        let hash: Option<u64> = value.hash.as_ref().map(hash_vec8_to_u64).transpose()?;
        let coords = match (value.gps_latitude, value.gps_longitude) {
            (Some(lat), Some(lon)) => Some(GpsCoordinates { lat, lon }),
            (None, None) => None,
            _ => {
                tracing::warn!(
                    asset_id = value.asset_id,
                    "Asset has only one of gps lat/lon"
                );
                None
            }
        };
        let base = AssetBase {
            id: AssetId(value.asset_id),
            ty: from_db_asset_ty(value.ty)?,
            root_dir_id: AssetRootDirId(value.root_dir_id),
            file_type: value.file_type,
            file_path: value.file_path.into(),
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
        };
        let sp = match ty {
            AssetType::Image => AssetSpe::Image(Image {
                image_format_name: value
                    .image_format_name
                    .ok_or(eyre!("image DbAsset must have image_format_name set"))?,
            }),
            AssetType::Video => AssetSpe::Video(Video {
                video_codec_name: value
                    .video_codec_name
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
        Ok(Asset { base, sp })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Insertable)]
#[diesel(table_name = super::super::schema::Asset)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbInsertAsset<'a> {
    pub asset_id: Option<i64>,
    pub ty: i32,
    pub root_dir_id: i64,
    pub file_type: Cow<'a, str>,
    pub file_path: Cow<'a, str>,
    pub is_hidden: i32,
    pub hash: Option<Cow<'a, [u8]>>,
    pub added_at: i64,
    pub taken_date: i64,
    pub timezone_offset: Option<Cow<'a, str>>,
    pub timezone_info: i32,
    pub width: i32,
    pub height: i32,
    pub rotation_correction: Option<i32>,
    pub exiftool_output: Cow<'a, [u8]>,
    pub gps_latitude: Option<i64>,
    pub gps_longitude: Option<i64>,

    pub motion_photo: i32,
    pub motion_photo_assoc_asset_id: Option<i64>,
    pub motion_photo_pts_us: Option<i64>,
    pub motion_photo_video_file_id: Option<i64>,

    pub image_format_name: Option<Cow<'a, str>>,
    pub ffprobe_output: Option<Cow<'a, [u8]>>,
    pub video_codec_name: Option<Cow<'a, str>>,
    pub video_bitrate: Option<i64>,
    pub video_duration_ms: Option<i64>,
    pub audio_codec_name: Option<Cow<'a, str>>,
    pub has_dash: Option<i32>,
}

pub fn to_db_asset_ty(ty: AssetType) -> i32 {
    match ty {
        AssetType::Image => 1,
        AssetType::Video => 2,
    }
}

pub fn from_db_asset_ty(i: i32) -> Result<AssetType> {
    match i {
        1 => Ok(AssetType::Image),
        2 => Ok(AssetType::Video),
        _ => Err(eyre!("Invalid column ty in Asset row")),
    }
}

// TODO roundtrip proptests making sure that composition of these is identity
pub fn to_db_timezone_info(tzi: &TimestampInfo) -> i32 {
    match tzi {
        TimestampInfo::TzCertain(_) => 1,
        TimestampInfo::UtcCertain => 2,
        TimestampInfo::TzSetByUser(_) => 3,
        TimestampInfo::TzInferredLocation(_) => 4,
        TimestampInfo::TzGuessedLocal(_) => 5,
        TimestampInfo::NoTimestamp => 6,
    }
}

fn from_db_timezone_info(i: i32, tz_offset: Option<&str>) -> Result<TimestampInfo> {
    match (i, tz_offset) {
        (1 | 3 | 4 | 5, Some(tz_offset)) => {
            let offset: FixedOffset = tz_offset
                .parse()
                .wrap_err("could not parse timezone offset")?;
            match i {
                1 => Ok(TimestampInfo::TzCertain(offset)),
                3 => Ok(TimestampInfo::TzSetByUser(offset)),
                4 => Ok(TimestampInfo::TzInferredLocation(offset)),
                5 => Ok(TimestampInfo::TzGuessedLocal(offset)),
                _ => unreachable!(),
            }
        }
        (2, _) => Ok(TimestampInfo::UtcCertain),
        (6, _) => Ok(TimestampInfo::NoTimestamp),
        _ => Err(eyre!("invalid timezone_info combination in db row ")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Queryable)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAssetPathOnDisk {
    #[diesel(column_name = asset_id)]
    pub asset_id: i64,
    #[diesel(column_name = path_in_asset_root)]
    pub path_in_asset_root: String,
    #[diesel(column_name = asset_root_path)]
    pub asset_root_path: String,
}

impl TryFrom<DbAssetPathOnDisk> for AssetPathOnDisk {
    type Error = eyre::Report;

    fn try_from(value: DbAssetPathOnDisk) -> Result<Self, Self::Error> {
        Ok(AssetPathOnDisk {
            id: AssetId(value.asset_id),
            path_in_asset_root: PathBuf::from(value.path_in_asset_root),
            asset_root_path: PathBuf::from(value.asset_root_path),
        })
    }
}
