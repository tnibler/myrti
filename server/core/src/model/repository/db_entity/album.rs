use crate::model::{util::datetime_from_db_repr, Album, AlbumId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DbAlbum {
    pub id: AlbumId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: i64,
    pub changed_at: i64,
    pub num_assets: Option<i64>,
}

impl TryFrom<&DbAlbum> for Album {
    type Error = eyre::Report;

    fn try_from(value: &DbAlbum) -> Result<Self, Self::Error> {
        let created_at = datetime_from_db_repr(value.created_at)?;
        let changed_at = datetime_from_db_repr(value.changed_at)?;
        Ok(Album {
            id: value.id,
            name: value.name.clone(),
            description: value.description.clone(),
            created_at,
            changed_at,
        })
    }
}

impl TryFrom<DbAlbum> for Album {
    type Error = eyre::Report;

    fn try_from(value: DbAlbum) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
