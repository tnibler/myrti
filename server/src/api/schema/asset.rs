use super::{AssetMetadata, AssetRootId};
use crate::model;
use chrono::{DateTime, Utc};
use eyre::bail;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetType {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: AssetId,
    pub asset_root_id: AssetRootId,
    pub path_in_root: PathBuf,
    #[serde(rename = "type")]
    pub ty: AssetType,
    pub width: i32,
    pub height: i32,
    pub added_at: DateTime<Utc>,
    pub taken_date: DateTime<Utc>,
    pub metadata: Option<AssetMetadata>,
}

impl From<model::Asset> for Asset {
    fn from(value: model::Asset) -> Self {
        Asset {
            id: value.base.id.into(),
            asset_root_id: value.base.root_dir_id.into(),
            path_in_root: value.base.file_path,
            ty: value.base.ty.into(),
            width: value.base.size.width as i32,
            height: value.base.size.height as i32,
            added_at: value.base.added_at,
            taken_date: value.base.taken_date,
            metadata: None,
        }
    }
}

impl From<model::AssetId> for AssetId {
    fn from(value: model::AssetId) -> Self {
        AssetId(value.0.to_string())
    }
}

impl TryFrom<AssetId> for model::AssetId {
    type Error = eyre::Report;
    fn try_from(value: AssetId) -> Result<Self, Self::Error> {
        match value.0.parse::<i64>() {
            Ok(id) => Ok(model::AssetId(id)),
            Err(_) => bail!("Invalid AssetId {}", value.0),
        }
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
