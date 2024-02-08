use camino::Utf8PathBuf as PathBuf;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::{schema, ToSchema};

use core::model;

use super::{AssetId, AssetMetadata, AssetRootDirId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum AssetType {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: AssetId,
    pub asset_root_id: AssetRootDirId,
    pub path_in_root: String,
    #[serde(rename = "type")]
    pub ty: AssetType,
    pub width: i32,
    pub height: i32,
    pub added_at: DateTime<Utc>,
    pub taken_date: DateTime<Utc>,
    pub metadata: Option<AssetMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetWithSpe {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(flatten)]
    pub spe: AssetSpe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(untagged)]
pub enum AssetSpe {
    Image(Image),
    Video(Video),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub representations: Vec<ImageRepresentation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageRepresentation {
    pub id: String,
    pub format: String,
    pub width: i32,
    pub height: i32,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Video {}

impl From<&model::Asset> for Asset {
    fn from(value: &model::Asset) -> Self {
        Asset {
            id: value.base.id.into(),
            asset_root_id: value.base.root_dir_id.into(),
            path_in_root: value.base.file_path.to_string(),
            ty: value.base.ty.into(),
            width: value.base.size.width as i32,
            height: value.base.size.height as i32,
            added_at: value.base.added_at,
            taken_date: value.base.taken_date,
            metadata: None,
        }
    }
}

impl From<model::Asset> for Asset {
    fn from(value: model::Asset) -> Self {
        (&value).into()
    }
}

impl From<AssetType> for model::AssetType {
    fn from(value: AssetType) -> Self {
        match value {
            AssetType::Image => model::AssetType::Image,
            AssetType::Video => model::AssetType::Video,
        }
    }
}

impl From<model::AssetType> for AssetType {
    fn from(value: model::AssetType) -> Self {
        match value {
            model::AssetType::Image => AssetType::Image,
            model::AssetType::Video => AssetType::Video,
        }
    }
}
