use super::{db_entity::DbAsset, AssetId, AssetRootDirId, AssetType, ResourceFileId};
use chrono::{DateTime, Utc};
use eyre::{eyre, Result};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetBase {
    pub id: AssetId,
    pub ty: AssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_path: PathBuf,
    pub file_created_at: Option<DateTime<Utc>>,
    pub file_modified_at: Option<DateTime<Utc>>,
    pub added_at: DateTime<Utc>,
    /// The date under which this asset is displayed in the timeline
    /// None: exif data not yet processed
    /// Some: set after processing exif data, either there is a date in there
    /// or we use file_created_at, file_modified_at or added_at in that order
    pub canonical_date: Option<DateTime<Utc>>,
    pub size: Size,
    /// Seahash of the file, if already computed
    pub hash: Option<Vec<u8>>,
    pub thumb_small_square_jpg: Option<ResourceFileId>,
    pub thumb_small_square_webp: Option<ResourceFileId>,
    pub thumb_large_orig_jpg: Option<ResourceFileId>,
    pub thumb_large_orig_webp: Option<ResourceFileId>,
    pub thumb_small_square_size: Option<Size>,
    pub thumb_large_orig_size: Option<Size>,
}

impl TryFrom<&AssetBase> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: &AssetBase) -> Result<Self, Self::Error> {
        let file_path = value
            .file_path
            .to_str()
            .ok_or_else(|| eyre!("non unicode file path not supported"))?
            .to_string();
        Ok(DbAsset {
            id: value.id,
            ty: value.ty.into(),
            root_dir_id: value.root_dir_id,
            file_path,
            hash: value.hash.clone(),
            added_at: value.added_at.naive_utc(),
            file_created_at: value.file_created_at.map(|t| t.naive_utc()),
            file_modified_at: value.file_modified_at.map(|t| t.naive_utc()),
            canonical_date: value.canonical_date.map(|t| t.naive_utc()),
            width: value.size.width,
            height: value.size.height,
            thumb_small_square_jpg: value.thumb_small_square_jpg,
            thumb_small_square_webp: value.thumb_small_square_webp,
            thumb_large_orig_jpg: value.thumb_large_orig_jpg,
            thumb_large_orig_webp: value.thumb_large_orig_webp,
            thumb_small_square_width: value.thumb_small_square_size.as_ref().map(|s| s.width),
            thumb_small_square_height: value.thumb_small_square_size.as_ref().map(|s| s.height),
            thumb_large_orig_width: value.thumb_large_orig_size.as_ref().map(|s| s.width),
            thumb_large_orig_height: value.thumb_large_orig_size.as_ref().map(|s| s.height),
        })
    }
}

impl TryFrom<AssetBase> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: AssetBase) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&DbAsset> for AssetBase {
    type Error = eyre::Report;

    fn try_from(value: &DbAsset) -> Result<Self, Self::Error> {
        Ok(AssetBase {
            id: value.id,
            ty: value.ty.into(),
            root_dir_id: value.root_dir_id,
            file_path: value.file_path.as_str().into(),
            added_at: value.added_at.and_utc(),
            file_created_at: value.file_created_at.map(|t| t.and_utc()),
            file_modified_at: value.file_modified_at.map(|t| t.and_utc()),
            hash: value.hash.clone(),
            canonical_date: value.canonical_date.map(|t| t.and_utc()),
            size: Size {
                width: value.width,
                height: value.height,
            },
            thumb_small_square_jpg: value.thumb_small_square_jpg,
            thumb_small_square_webp: value.thumb_small_square_webp,
            thumb_large_orig_jpg: value.thumb_large_orig_jpg,
            thumb_large_orig_webp: value.thumb_large_orig_webp,
            thumb_small_square_size: value
                .thumb_small_square_width
                .zip(value.thumb_small_square_height)
                .map(|(width, height)| Size { width, height }),
            thumb_large_orig_size: value
                .thumb_large_orig_width
                .zip(value.thumb_large_orig_height)
                .map(|(width, height)| Size { width, height }),
        })
    }
}

impl TryFrom<DbAsset> for AssetBase {
    type Error = eyre::Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
