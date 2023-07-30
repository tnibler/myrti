use super::{db_entity::DbAsset, AssetId, AssetRootDirId, AssetType};
use chrono::{DateTime, Utc};
use eyre::{eyre, Result};
use serde::Serialize;
use std::path::PathBuf;

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
    /// Seahash of the file, if already computed
    pub hash: Option<Vec<u8>>,
    pub thumb_path_small_square_jpg: Option<PathBuf>,
    pub thumb_path_small_square_webp: Option<PathBuf>,
    pub thumb_path_large_orig_jpg: Option<PathBuf>,
    pub thumb_path_large_orig_webp: Option<PathBuf>,
}

fn opt_path_to_string(path: &Option<PathBuf>) -> Result<Option<String>> {
    match path.as_ref() {
        None => Ok(None),
        Some(p) => Ok(Some(
            p.to_str()
                .ok_or_else(|| eyre!("non unicode file path not supported"))?
                .to_string(),
        )),
    }
}

impl TryFrom<&AssetBase> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: &AssetBase) -> Result<Self, Self::Error> {
        let file_path = value
            .file_path
            .to_str()
            .ok_or_else(|| eyre!("non unicode file path not supported"))?
            .to_string();
        let thumb_path_small_square_jpg = opt_path_to_string(&value.thumb_path_small_square_jpg)?;
        let thumb_path_small_square_webp = opt_path_to_string(&value.thumb_path_small_square_webp)?;
        let thumb_path_large_orig_jpg = opt_path_to_string(&value.thumb_path_large_orig_jpg)?;
        let thumb_path_large_orig_webp = opt_path_to_string(&value.thumb_path_large_orig_webp)?;
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
            thumb_path_small_square_jpg,
            thumb_path_small_square_webp,
            thumb_path_large_orig_jpg,
            thumb_path_large_orig_webp,
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
            thumb_path_small_square_jpg: value
                .thumb_path_small_square_jpg
                .as_ref()
                .map(|p| p.into()),
            thumb_path_small_square_webp: value
                .thumb_path_small_square_webp
                .as_ref()
                .map(|p| p.into()),
            thumb_path_large_orig_jpg: value.thumb_path_large_orig_jpg.as_ref().map(|p| p.into()),
            thumb_path_large_orig_webp: value.thumb_path_large_orig_webp.as_ref().map(|p| p.into()),
        })
    }
}

impl TryFrom<DbAsset> for AssetBase {
    type Error = eyre::Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
