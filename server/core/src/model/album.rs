use chrono::{DateTime, Utc};

use super::{AlbumId, AlbumItemId, Asset};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Album {
    pub id: AlbumId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub changed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlbumItem {
    pub id: AlbumItemId,
    pub item: AlbumItemType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AlbumItemType {
    Asset(Asset),
    Text(String),
}
