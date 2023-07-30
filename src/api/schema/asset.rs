use super::{AssetMetadata, AssetRootId};
use crate::model;
use chrono::{DateTime, Utc};
use eyre::{bail, eyre};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum AssetType {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Asset {
    pub id: AssetId,
    pub asset_root_id: AssetRootId,
    pub path_in_root: PathBuf,
    #[serde(rename = "type")]
    pub ty: AssetType,
    pub file_created_at: Option<DateTime<Utc>>,
    pub file_modified_at: Option<DateTime<Utc>>,
    pub added_at: DateTime<Utc>,
    pub metadata: Option<AssetMetadata>,
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
