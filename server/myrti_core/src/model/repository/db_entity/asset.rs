use std::borrow::Cow;

use camino::Utf8PathBuf as PathBuf;
use chrono::FixedOffset;
use diesel::prelude::*;
use eyre::{Context, Result, eyre};

use crate::model::{
    AssetBase, AssetFile, AssetId, AssetPathOnDisk, AssetRootDirId, AssetSeriesId, AssetType,
    FileId, GpsCoordinates, Image, InSeries, IsOriginalStreamable, MirrorCorrection,
    OriginalStreaming, RotationCorrection, Size, TimestampInfo, Video,
    util::{datetime_from_db_repr, hash_vec8_to_u64},
};

#[derive(
    Debug, Clone, PartialEq, Eq, Identifiable, Queryable, QueryableByName, Selectable, Associations,
)]
#[diesel(table_name = super::super::schema::AssetFile)]
#[diesel(primary_key(file_id))]
#[diesel(belongs_to(DbAsset, foreign_key = asset_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAssetFile {
    pub file_id: i64,
    pub asset_id: i64,
    pub asset_type: i32,
    pub root_dir_id: i64,
    pub file_type: String,
    pub file_path: String,
    pub hash: Option<Vec<u8>>,
    pub added_at: i64,
    pub width: i32,
    pub height: i32,
    pub rotation_correction: i32,
    pub mirror_correction: i32,
    pub thumb_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Identifiable, Queryable, QueryableByName, Selectable)]
#[diesel(table_name = super::super::schema::Asset)]
#[diesel(primary_key(asset_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAsset {
    pub asset_id: i64,
    pub asset_type: i32,
    pub rep_file_id: i64,
    pub series_id: Option<i64>,
    pub is_series_selection: Option<i32>,
    pub is_hidden: i32,
    pub taken_date: i64,
    pub timezone_offset: Option<String>,
    pub timezone_info: i32,
    pub gps_latitude: Option<i64>,
    pub gps_longitude: Option<i64>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Identifiable, Queryable, QueryableByName, Selectable, Associations,
)]
#[diesel(table_name = super::super::schema::ImageFile)]
#[diesel(primary_key(file_id))]
#[diesel(belongs_to(DbAssetFile, foreign_key = file_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbImageFile {
    pub file_id: i64,
    pub image_format_name: String,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Identifiable, Queryable, QueryableByName, Selectable, Associations,
)]
#[diesel(table_name = super::super::schema::VideoFile)]
#[diesel(primary_key(file_id))]
#[diesel(belongs_to(DbAssetFile, foreign_key = file_id))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbVideoFile {
    pub file_id: i64,
    pub video_codec_name: String,
    pub video_bitrate: Option<i64>,
    pub audio_codec_name: Option<String>,
    pub original_streaming: Option<i32>,
    pub original_streaming_state: Option<i32>,
    pub ghi_disabled: i32,
    pub max_iframe_interval: Option<i32>,
    pub frame_rate_num: Option<i32>,
    pub frame_rate_denom: Option<i32>,
}

impl TryFrom<DbAssetFile> for AssetFile {
    type Error = eyre::Report;

    fn try_from(value: DbAssetFile) -> Result<Self, Self::Error> {
        let hash: Option<u64> = value.hash.as_ref().map(hash_vec8_to_u64).transpose()?;
        Ok(AssetFile {
            id: FileId(value.file_id),
            ty: from_db_asset_ty(value.asset_type)?,
            root_dir_id: AssetRootDirId(value.root_dir_id),
            file_type: value.file_type,
            file_path: value.file_path.into(),
            added_at: datetime_from_db_repr(value.added_at)?,
            hash,
            size: Size {
                width: value.width,
                height: value.height,
            },
            rotation_correction: match value.rotation_correction {
                0 => RotationCorrection::CW0,
                1 => RotationCorrection::CW90,
                2 => RotationCorrection::CW180,
                3 => RotationCorrection::CW270,
                other => {
                    return Err(eyre!(
                        "Invalid value {} in column AssetFile.rotation_correction",
                        other
                    ));
                }
            },
            mirror_correction: match value.mirror_correction {
                0 => MirrorCorrection::None,
                1 => MirrorCorrection::Horizontal,
                2 => MirrorCorrection::Vertical,
                other => {
                    return Err(eyre!(
                        "Invalid value {} in column AssetFile.mirror_correction",
                        other
                    ));
                }
            },
            thumbhash: value.thumb_hash,
        })
    }
}

impl TryFrom<DbAsset> for AssetBase {
    type Error = eyre::Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        let timestamp_info =
            from_db_timezone_info(value.timezone_info, value.timezone_offset.as_deref())?;
        let coords = match (value.gps_latitude, value.gps_longitude) {
            (Some(lat), Some(lon)) => Some(GpsCoordinates { lat, lon }),
            (None, None) => None,
            _ => {
                panic!("Asset only has one of gps lat/lon, db constraints should disallow this")
            }
        };
        let in_series = match (value.series_id, value.is_series_selection) {
            (Some(series_id), Some(is_selection)) => Some(InSeries {
                series_id: AssetSeriesId(series_id),
                is_selection: is_selection != 0,
            }),
            (None, None) => None,
            _ => {
                panic!(
                    "Asset only has one of series_id and is_series_selection, db constraints should disallow this"
                )
            }
        };
        Ok(AssetBase {
            id: AssetId(value.asset_id),
            rep_file_id: FileId(value.rep_file_id),
            ty: from_db_asset_ty(value.asset_type)?,
            is_hidden: value.is_hidden != 0,
            taken_date: datetime_from_db_repr(value.taken_date)?,
            timestamp_info,
            gps_coordinates: coords,
            in_series,
        })
    }
}

impl TryFrom<DbVideoFile> for Video {
    type Error = eyre::Report;

    fn try_from(value: DbVideoFile) -> Result<Self, Self::Error> {
        let original_streamable = value
            .original_streaming
            .map(IsOriginalStreamable::try_from)
            .transpose()?;
        Ok(Video {
            file_id: FileId(value.file_id),
            video_codec_name: value.video_codec_name,
            video_bitrate: value.video_bitrate,
            audio_codec_name: value.audio_codec_name,
            max_iframe_interval: value.max_iframe_interval,
            original_streaming: match (
                original_streamable,
                value.ghi_disabled,
                value.original_streaming_state,
            ) {
                (Some(s), 1, Some(done)) if done == 0 || done == 2 => Some(OriginalStreaming {
                    is_streamable: s,
                    is_done: done == 2,
                }),
                (Some(s), 0, Some(done)) if done == 0 || done == 1 => Some(OriginalStreaming {
                    is_streamable: s,
                    is_done: done == 1,
                }),
                (Some(s @ IsOriginalStreamable::None), 0, None) => Some(OriginalStreaming {
                    is_streamable: s,
                    is_done: false,
                }),
                (None, _, None) => None,
                (s, g, done) => {
                    return Err(eyre!(
                        "invalid original_streaming columns; {s:?}, {g:?}, {done:?}"
                    ));
                }
            },
            ghi_disabled: value.ghi_disabled != 0,
            frame_rate: value
                .frame_rate_num
                .and_then(|num| value.frame_rate_denom.map(|denom| (num, denom))),
        })
    }
}

impl TryFrom<DbImageFile> for Image {
    type Error = eyre::Report;

    fn try_from(value: DbImageFile) -> Result<Self, Self::Error> {
        Ok(Image {
            file_id: FileId(value.file_id),
            image_format_name: value.image_format_name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Insertable)]
#[diesel(table_name = super::super::schema::AssetFile)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbInsertAssetFile<'a> {
    pub file_id: Option<i64>,
    pub asset_id: Option<i64>,
    pub asset_type: i32,
    pub root_dir_id: i64,
    pub file_type: Cow<'a, str>,
    pub file_path: Cow<'a, str>,
    pub hash: Option<Cow<'a, [u8]>>,
    pub added_at: i64,

    pub width: i32,
    pub height: i32,
    pub rotation_correction: i32,
    pub exiftool_output: Cow<'a, [u8]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Insertable)]
#[diesel(table_name = super::super::schema::Asset)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbInsertAsset<'a> {
    pub asset_id: Option<i64>,
    pub asset_type: i32,
    pub rep_file_id: Option<i64>,
    pub is_hidden: i32,
    pub taken_date: i64,
    pub timezone_offset: Option<Cow<'a, str>>,
    pub timezone_info: i32,
    pub gps_latitude: Option<i64>,
    pub gps_longitude: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Insertable)]
#[diesel(table_name = super::super::schema::VideoFile)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbInsertVideoFile<'a> {
    pub file_id: i64,
    pub ffprobe_output: Cow<'a, [u8]>,
    pub video_codec_name: Cow<'a, str>,
    pub video_bitrate: Option<i64>,
    pub video_duration_ms: Option<i64>,
    pub audio_codec_name: Option<Cow<'a, str>>,
    pub original_streaming: Option<i32>,
    pub ghi_disabled: i32,
    pub original_streaming_state: Option<i32>,
    pub max_iframe_interval: Option<i32>,
    pub frame_rate_num: Option<i32>,
    pub frame_rate_denom: Option<i32>,
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
    #[diesel(column_name = file_id)]
    pub file_id: i64,
    #[diesel(column_name = path_in_asset_root)]
    pub path_in_asset_root: String,
    #[diesel(column_name = asset_root_path)]
    pub asset_root_path: String,
}

impl TryFrom<DbAssetPathOnDisk> for AssetPathOnDisk {
    type Error = eyre::Report;

    fn try_from(value: DbAssetPathOnDisk) -> Result<Self, Self::Error> {
        Ok(AssetPathOnDisk {
            file_id: FileId(value.file_id),
            path_in_asset_root: PathBuf::from(value.path_in_asset_root),
            asset_root_path: PathBuf::from(value.asset_root_path),
        })
    }
}

impl TryFrom<i32> for IsOriginalStreamable {
    type Error = eyre::Report;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::VideoOnly),
            2 => Ok(Self::AudioOnly),
            3 => Ok(Self::VideoAudio),
            other => Err(eyre!("invalid IsOriginalStreamable value {}", other)),
        }
    }
}

impl From<IsOriginalStreamable> for i32 {
    fn from(value: IsOriginalStreamable) -> Self {
        match value {
            IsOriginalStreamable::None => 0,
            IsOriginalStreamable::VideoOnly => 1,
            IsOriginalStreamable::AudioOnly => 2,
            IsOriginalStreamable::VideoAudio => 3,
        }
    }
}
