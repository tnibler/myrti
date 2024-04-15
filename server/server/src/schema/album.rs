use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use core::model;

use super::AlbumId;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: AlbumId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub num_assets: i64,
    pub created_at: DateTime<Utc>,
    pub changed_at: DateTime<Utc>,
}

impl Album {
    pub fn from_model(value: &model::Album, num_assets: i64) -> Album {
        Album {
            id: value.id.into(),
            name: value.name.clone(),
            description: value.description.clone(),
            num_assets,
            created_at: value.created_at,
            changed_at: value.changed_at,
        }
    }
}
