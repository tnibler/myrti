use camino::Utf8PathBuf as PathBuf;
use chrono::{DateTime, Utc};
use eyre::bail;
use serde::Serialize;

use crate::model;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRootId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRoot {
    pub id: AssetRootId,
    pub path: PathBuf,
    pub num_assets: i32,
    pub last_full_reindex: Option<DateTime<Utc>>,
    pub last_change: Option<DateTime<Utc>>,
}

impl From<&model::AssetRootDirId> for AssetRootId {
    fn from(value: &model::AssetRootDirId) -> Self {
        AssetRootId(value.0.to_string())
    }
}

impl TryFrom<&AssetRootId> for model::AssetRootDirId {
    type Error = eyre::Report;
    fn try_from(value: &AssetRootId) -> Result<Self, Self::Error> {
        match value.0.parse::<i64>() {
            Ok(id) => Ok(model::AssetRootDirId(id)),
            Err(_) => bail!("Invalid AssetRootDirId {}", value.0),
        }
    }
}

impl From<model::AssetRootDirId> for AssetRootId {
    fn from(value: model::AssetRootDirId) -> Self {
        AssetRootId(value.0.to_string())
    }
}

impl TryFrom<AssetRootId> for model::AssetRootDirId {
    type Error = eyre::Report;
    fn try_from(value: AssetRootId) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
