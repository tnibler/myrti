use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::model;

use super::AlbumId;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: AlbumId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub num_assets: i64,
    pub created_at: DateTime<Utc>,
    pub changed_at: DateTime<Utc>,
    #[serde(rename = "type")]
    pub ty: AlbumType,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum AlbumType {
    TimelineGroup { display_date: DateTime<Utc> },
    Album,
}

impl Album {
    pub fn from_model(value: &model::AlbumType, num_assets: i64) -> Album {
        let base = value.album_base();
        Album {
            id: base.id.into(),
            name: base.name.clone(),
            description: base.description.clone(),
            num_assets,
            created_at: base.created_at.clone(),
            changed_at: base.changed_at.clone(),
            ty: match value {
                model::AlbumType::Album(_) => AlbumType::Album,
                model::AlbumType::TimelineGroup(tg) => AlbumType::TimelineGroup {
                    display_date: tg.group.display_date,
                },
            },
        }
    }
}
