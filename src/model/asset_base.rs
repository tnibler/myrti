use chrono::{DateTime, NaiveDateTime, Utc};
use eyre::{bail, eyre, Context, Result};
use serde::Serialize;
use std::path::PathBuf;

use super::{
    repository::db_entity::{DbAsset, DbAssetType},
    util::{
        datetime_from_db_repr, datetime_to_db_repr, hash_u64_to_vec8, hash_vec8_to_u64,
        opt_path_to_string, path_to_string,
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
    pub added_at: DateTime<Utc>,
    /// The date under which this asset is displayed in the timeline
    /// None: exif data not yet processed
    /// Some: set after processing exif data, either there is a date in there
    /// or we use file_created_at, file_modified_at or added_at in that order
    pub taken_date: MediaTimestamp,
    pub size: Size,
    /// degrees clockwise
    pub rotation_correction: Option<i32>,
    /// Seahash of the file, if already computed
    pub hash: Option<u64>,
    pub thumb_small_square_avif: Option<PathBuf>,
    pub thumb_small_square_webp: Option<PathBuf>,
    pub thumb_large_orig_avif: Option<PathBuf>,
    pub thumb_large_orig_webp: Option<PathBuf>,
    pub thumb_small_square_size: Option<Size>,
    pub thumb_large_orig_size: Option<Size>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Hash)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MediaTimestamp {
    Utc(DateTime<Utc>),
    LocalFallback(NaiveDateTime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbnailType {
    SmallSquare,
    LargeOrigAspect,
}

impl TryFrom<&Asset> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: &Asset) -> Result<Self, Self::Error> {
        let file_path = path_to_string(&value.base.file_path)?;
        let (taken_date, taken_date_local_fallback) = match value.base.taken_date {
            MediaTimestamp::Utc(with_offset) => (Some(datetime_to_db_repr(&with_offset)), None),
            MediaTimestamp::LocalFallback(naive) => (None, Some(naive.to_string())),
        };
        let video = match &value.sp {
            super::AssetSpe::Image(_) => None,
            super::AssetSpe::Video(video) => Some(video),
        };
        Ok(DbAsset {
            id: value.base.id,
            ty: value.base.ty.into(),
            root_dir_id: value.base.root_dir_id,
            file_type: value.base.file_type.clone(),
            file_path,
            hash: value.base.hash.map(|h| hash_u64_to_vec8(h)),
            added_at: datetime_to_db_repr(&value.base.added_at),
            taken_date,
            taken_date_local_fallback,
            width: value.base.size.width,
            height: value.base.size.height,
            rotation_correction: value.base.rotation_correction,
            thumb_small_square_avif: opt_path_to_string(&value.base.thumb_small_square_avif)?,
            thumb_small_square_webp: opt_path_to_string(&value.base.thumb_small_square_webp)?,
            thumb_large_orig_avif: opt_path_to_string(&value.base.thumb_large_orig_avif)?,
            thumb_large_orig_webp: opt_path_to_string(&value.base.thumb_large_orig_webp)?,
            thumb_small_square_width: value.base.thumb_small_square_size.as_ref().map(|s| s.width),
            thumb_small_square_height: value
                .base
                .thumb_small_square_size
                .as_ref()
                .map(|s| s.height),
            thumb_large_orig_width: value.base.thumb_large_orig_size.as_ref().map(|s| s.width),
            thumb_large_orig_height: value.base.thumb_large_orig_size.as_ref().map(|s| s.height),
            video_codec_name: video.map(|v| v.video_codec_name.clone()),
            video_bitrate: video.map(|v| v.video_bitrate),
            audio_codec_name: video.map(|v| v.audio_codec_name.clone()).flatten(),
            resource_dir: video
                .map(|v| v.dash_resource_dir.as_ref().map(|p| path_to_string(p)))
                .flatten()
                .transpose()?,
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
        let taken_date = match (&value.taken_date, &value.taken_date_local_fallback) {
            (Some(utc), _) => MediaTimestamp::Utc(
                datetime_from_db_repr(utc).wrap_err("could not parse taken_date")?,
            ),
            (None, Some(local)) => MediaTimestamp::LocalFallback(local.parse::<NaiveDateTime>()?),
            (None, None) => {
                bail!("one of taken_date or taken_date_local_fallback must be set in Assets row")
            }
        };
        let sp = match value.ty {
            DbAssetType::Image => AssetSpe::Image(Image {}),
            DbAssetType::Video => AssetSpe::Video(Video {
                video_codec_name: value
                    .video_codec_name
                    .clone()
                    .ok_or(eyre!("video DbAsset must have video_codec_name set"))?,
                video_bitrate: value
                    .video_bitrate
                    .ok_or(eyre!("video DbAsset must have video_bitrate set"))?,
                audio_codec_name: value.audio_codec_name.clone(),
                dash_resource_dir: value.resource_dir.as_ref().map(|p| PathBuf::from(p)),
            }),
        };
        let hash: Option<u64> = value
            .hash
            .as_ref()
            .map(|a| hash_vec8_to_u64(&a))
            .transpose()?;
        Ok(Asset {
            sp,
            base: AssetBase {
                id: value.id,
                ty: value.ty.into(),
                root_dir_id: value.root_dir_id,
                file_type: value.file_type.clone(),
                file_path: value.file_path.as_str().into(),
                added_at: datetime_from_db_repr(&value.added_at)?,
                hash,
                taken_date,
                size: Size {
                    width: value.width,
                    height: value.height,
                },
                rotation_correction: value.rotation_correction,
                thumb_small_square_avif: value.thumb_small_square_avif.as_ref().map(|p| p.into()),
                thumb_small_square_webp: value.thumb_small_square_webp.as_ref().map(|p| p.into()),
                thumb_large_orig_avif: value.thumb_large_orig_avif.as_ref().map(|p| p.into()),
                thumb_large_orig_webp: value.thumb_large_orig_webp.as_ref().map(|p| p.into()),
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
