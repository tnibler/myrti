use std::borrow::Cow;

use diesel::{prelude::Insertable, Queryable, Selectable};

use crate::model::{util::datetime_from_db_repr, Album, AlbumId};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::Album)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAlbum {
    pub album_id: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: i64,
    pub changed_at: i64,
}

impl TryFrom<DbAlbum> for Album {
    type Error = eyre::Report;

    fn try_from(value: DbAlbum) -> Result<Self, Self::Error> {
        let created_at = datetime_from_db_repr(value.created_at)?;
        let changed_at = datetime_from_db_repr(value.changed_at)?;
        Ok(Album {
            id: AlbumId(value.album_id),
            name: value.name,
            description: value.description,
            created_at,
            changed_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Queryable)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAlbumWithAssetCount {
    #[diesel(embed)]
    pub album: DbAlbum,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub asset_count: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = super::super::schema::Album)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbInsertAlbum<'a> {
    pub album_id: Option<i64>,
    pub name: Option<Cow<'a, str>>,
    pub description: Option<Cow<'a, str>>,
    pub created_at: i64,
    pub changed_at: i64,
}
