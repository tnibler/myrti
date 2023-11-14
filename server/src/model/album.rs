use chrono::{DateTime, Utc};

use super::AlbumId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineGroup {
    pub display_date: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Album {
    pub id: AlbumId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub changed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineGroupAlbum {
    pub album: Album,
    pub group: TimelineGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AlbumType {
    Album(Album),
    TimelineGroup(TimelineGroupAlbum),
}

impl AlbumType {
    pub fn album_base(&self) -> &Album {
        match self {
            AlbumType::Album(a) => a,
            AlbumType::TimelineGroup(tg) => &tg.album,
        }
    }
}
