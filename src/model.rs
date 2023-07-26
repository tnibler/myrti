use chrono::{DateTime, Utc};
use color_eyre::eyre;
use serde::Serialize;
use std::{fmt::Display, path::PathBuf};

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Copy, Serialize)]
#[sqlx(transparent)]
pub struct AssetRootDirId(pub i64);

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Copy, Serialize)]
#[sqlx(transparent)]
pub struct AssetId(pub i64);

impl From<i64> for AssetRootDirId {
    fn from(value: i64) -> Self {
        AssetRootDirId(value)
    }
}

impl From<i64> for AssetId {
    fn from(value: i64) -> Self {
        AssetId(value)
    }
}

impl Display for AssetRootDirId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("AssetRootDirId({})", self.0))
    }
}

impl Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("AssetId({})", self.0))
    }
}

pub mod entity {
    use super::{AssetId, AssetRootDirId};
    use chrono::{DateTime, NaiveDateTime, Utc};

    #[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(i32)]
    pub enum DbAssetType {
        Image = 1,
        Video = 2,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DbAssetRootDir {
        pub id: AssetRootDirId,
        pub path: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DbAsset {
        pub id: AssetId,
        pub ty: DbAssetType,
        pub root_dir_id: AssetRootDirId,
        pub file_path: String,
        pub file_created_at: NaiveDateTime,
        pub file_modified_at: NaiveDateTime,
        pub thumb_path_jpg: Option<String>,
        pub thumb_path_webp: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DbVideoInfo {
        pub asset_id: AssetId,
        pub dash_manifest_path: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DbImageInfo {
        pub asset_id: AssetId,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRootDir {
    pub id: AssetRootDirId,
    pub path: PathBuf,
}

impl From<entity::DbAssetRootDir> for AssetRootDir {
    fn from(value: entity::DbAssetRootDir) -> Self {
        AssetRootDir {
            id: value.id,
            path: PathBuf::from(value.path),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum AssetType {
    Image,
    Video,
}

impl From<entity::DbAssetType> for AssetType {
    fn from(value: entity::DbAssetType) -> Self {
        match value {
            entity::DbAssetType::Image => AssetType::Image,
            entity::DbAssetType::Video => AssetType::Video,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetBase {
    pub id: AssetId,
    pub ty: AssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_path: PathBuf,
    pub file_created_at: DateTime<Utc>,
    pub file_modified_at: DateTime<Utc>,
    pub thumb_path_jpg: Option<PathBuf>,
    pub thumb_path_webp: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asset {
    Image {},
    Video { dash_manifest_path: Option<PathBuf> },
}

pub struct FullAsset {
    pub base: AssetBase,
    pub asset: Asset,
}

impl From<entity::DbAsset> for AssetBase {
    fn from(value: entity::DbAsset) -> Self {
        AssetBase {
            id: value.id,
            ty: value.ty.into(),
            root_dir_id: value.root_dir_id,
            file_path: value.file_path.into(),
            file_created_at: value.file_created_at.and_utc(),
            file_modified_at: value.file_modified_at.and_utc(),
            thumb_path_jpg: value.thumb_path_jpg.map(|p| p.into()),
            thumb_path_webp: value.thumb_path_webp.map(|p| p.into()),
        }
    }
}
