use eyre::eyre;

use crate::model::{
    util::datetime_from_db_repr, Album, AlbumId, AlbumType, TimelineGroup, TimelineGroupAlbum,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DbAlbum {
    pub id: AlbumId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_timeline_group: i64,
    pub timeline_group_display_date: Option<i64>,
    pub created_at: i64,
    pub changed_at: i64,
    pub num_assets: Option<i64>,
}

impl TryFrom<&DbAlbum> for AlbumType {
    type Error = eyre::Report;

    fn try_from(value: &DbAlbum) -> Result<Self, Self::Error> {
        let created_at = datetime_from_db_repr(value.created_at)?;
        let changed_at = datetime_from_db_repr(value.changed_at)?;
        let album_base = Album {
            id: value.id,
            name: value.name.clone(),
            description: value.description.clone(),
            created_at,
            changed_at,
        };
        let tg = match value.is_timeline_group {
            0 => None,
            _ => {
                let display_date = value
                    .timeline_group_display_date
                    .ok_or(eyre!(
                        "timeline_group_display_date is NULL but is_timeline_group!=0"
                    ))
                    .map(|d| datetime_from_db_repr(d))??;
                Some(TimelineGroup { display_date })
            }
        };
        match tg {
            None => Ok(AlbumType::Album(album_base)),
            Some(tg) => Ok(AlbumType::TimelineGroup(TimelineGroupAlbum {
                album: album_base,
                group: tg,
            })),
        }
    }
}

impl TryFrom<DbAlbum> for AlbumType {
    type Error = eyre::Report;

    fn try_from(value: DbAlbum) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&DbAlbum> for TimelineGroupAlbum {
    type Error = eyre::Report;

    fn try_from(value: &DbAlbum) -> Result<Self, Self::Error> {
        match AlbumType::try_from(value)? {
            AlbumType::Album(_) => Err(eyre!("Album is not a TimelineGroup")),
            AlbumType::TimelineGroup(g) => Ok(g),
        }
    }
}
