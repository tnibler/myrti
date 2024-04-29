use diesel::{Queryable, Selectable};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::AlbumItem)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAlbumItem {
    pub album_item_id: i64,
    pub ty: i32,
    pub asset_id: Option<i64>,
    pub text: Option<String>,
    pub idx: i32,
}
