use super::{db_entity::DbAsset, AssetId, AssetRootDirId, AssetType};
use chrono::{DateTime, Utc};
use eyre::eyre;
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
    pub thumb_path_jpg: Option<PathBuf>,
    pub thumb_path_webp: Option<PathBuf>,
}

impl TryFrom<&AssetBase> for DbAsset {
    type Error = eyre::Report;

    fn try_from(value: &AssetBase) -> Result<Self, Self::Error> {
        let file_path = value
            .file_path
            .to_str()
            .ok_or_else(|| eyre!("non unicode file path not supported"))?
            .to_string();
        let thumb_path_jpg = match value.thumb_path_jpg.as_ref() {
            None => None,
            Some(p) => Some(
                p.to_str()
                    .ok_or_else(|| eyre!("non unicode file path not supported"))?
                    .to_string(),
            ),
        };
        let thumb_path_webp = match value.thumb_path_webp.as_ref() {
            None => None,
            Some(p) => Some(
                p.to_str()
                    .ok_or_else(|| eyre!("non unicode file path not supported"))?
                    .to_string(),
            ),
        };
        Ok(DbAsset {
            id: value.id,
            ty: value.ty.into(),
            root_dir_id: value.root_dir_id,
            file_path,
            file_created_at: value.file_created_at.map(|t| t.naive_utc()),
            file_modified_at: value.file_modified_at.map(|t| t.naive_utc()),
            thumb_path_jpg,
            thumb_path_webp,
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
            file_created_at: value.file_created_at.map(|t| t.and_utc()),
            file_modified_at: value.file_modified_at.map(|t| t.and_utc()),
            thumb_path_jpg: value.thumb_path_jpg.as_ref().map(|p| p.into()),
            thumb_path_webp: value.thumb_path_webp.as_ref().map(|p| p.into()),
        })
    }
}

impl TryFrom<DbAsset> for AssetBase {
    type Error = eyre::Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
