use chrono::{DateTime, Utc};

use super::AlbumId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Album {
    pub id: AlbumId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub changed_at: DateTime<Utc>,
}

