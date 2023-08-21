use chrono::{DateTime, NaiveDateTime, Utc};
use eyre::{bail, eyre, Result};
use serde::Serialize;
use std::path::PathBuf;

use super::{
    repository::db_entity::{DbAsset, DbAssetType},
    util::{opt_path_to_string, path_to_string},
    Asset, AssetId, AssetRootDirId, AssetSpe, AssetType, Image, Video,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetBase {
    pub id: AssetId,
    pub ty: AssetType,
    pub root_dir_id: AssetRootDirId,
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
    pub hash: Option<Vec<u8>>,
    pub thumb_small_square_avif: Option<PathBuf>,
    pub thumb_small_square_webp: Option<PathBuf>,
    pub thumb_large_orig_avif: Option<PathBuf>,
    pub thumb_large_orig_webp: Option<PathBuf>,
    pub thumb_small_square_size: Option<Size>,
    pub thumb_large_orig_size: Option<Size>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaTimestamp {
    Utc(DateTime<Utc>),
    LocalFallback(NaiveDateTime),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailType {
    SmallSquare,
    LargeOrigAspect,
}

impl TryFrom<&Asset> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: &Asset) -> Result<Self, Self::Error> {
        let file_path = path_to_string(&value.base.file_path)?;
        let (taken_date, taken_date_local_fallback) = match value.base.taken_date {
            MediaTimestamp::Utc(with_offset) => (Some(with_offset.naive_utc()), None),
            MediaTimestamp::LocalFallback(naive) => (None, Some(naive)),
        };
        let video = match &value.sp {
            super::AssetSpe::Image(_) => None,
            super::AssetSpe::Video(video) => Some(video),
        };
        Ok(DbAsset {
            id: value.base.id,
            ty: value.base.ty.into(),
            root_dir_id: value.base.root_dir_id,
            file_path,
            hash: value.base.hash.clone(),
            added_at: value.base.added_at.naive_utc(),
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
            codec_name: video.map(|v| v.codec_name.clone()),
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
        let taken_date = match (value.taken_date, value.taken_date_local_fallback) {
            (Some(naive_utc), _) => MediaTimestamp::Utc(naive_utc.and_utc()),
            (None, Some(local)) => MediaTimestamp::LocalFallback(local),
            (None, None) => {
                bail!("one of taken_date or taken_date_local_fallback must be set in Assets row")
            }
        };
        let sp = match value.ty {
            DbAssetType::Image => AssetSpe::Image(Image {}),
            DbAssetType::Video => AssetSpe::Video(Video {
                codec_name: value
                    .codec_name
                    .clone()
                    .ok_or(eyre!("video DbAsset must have codec_name set"))?,
                dash_resource_dir: value.resource_dir.as_ref().map(|p| PathBuf::from(p)),
            }),
        };
        Ok(Asset {
            sp,
            base: AssetBase {
                id: value.id,
                ty: value.ty.into(),
                root_dir_id: value.root_dir_id,
                file_path: value.file_path.as_str().into(),
                added_at: value.added_at.and_utc(),
                hash: value.hash.clone(),
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
