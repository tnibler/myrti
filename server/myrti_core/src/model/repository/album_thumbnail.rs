use diesel::prelude::*;
use eyre::{Context, Result};

use crate::model::{repository::schema, AlbumId, AlbumThumbnailId};

use super::db::DbConn;

#[derive(Debug, Clone)]
pub struct InsertAlbumThumbnail {
    pub album_id: AlbumId,
    pub format_name: String,
    pub size: i32,
    pub file_key: String,
}

#[tracing::instrument(skip(conn))]
pub fn insert_album_thumbnail(
    conn: &mut DbConn,
    insert: InsertAlbumThumbnail,
) -> Result<AlbumThumbnailId> {
    use schema::AlbumThumbnail;
    let thumbnail_id: i64 = diesel::insert_into(AlbumThumbnail::table)
        .values((
            AlbumThumbnail::album_id.eq(insert.album_id.0),
            AlbumThumbnail::width.eq(insert.size),
            AlbumThumbnail::height.eq(insert.size),
            AlbumThumbnail::format_name.eq(&insert.format_name),
            AlbumThumbnail::file_key.eq(&insert.file_key),
        ))
        .returning(AlbumThumbnail::thumbnail_id)
        .get_result(conn)
        .wrap_err("error inserting into table AlbumThumbnail")?;
    Ok(AlbumThumbnailId(thumbnail_id))
}

#[tracing::instrument(skip(conn))]
pub fn get_albums_with_missing_thumbnails(conn: &mut DbConn) -> Result<Vec<AlbumId>> {
    #[derive(Debug, Clone, QueryableByName)]
    #[diesel(check_for_backend(diesel::sqlite::Sqlite))]
    struct Row {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        pub album_id: i64,
    }
    let rows: Vec<Row> = diesel::sql_query(r#"
    SELECT Album.album_id 
    FROM Album
    WHERE
    (NOT EXISTS (SELECT * FROM AlbumThumbnail at WHERE at.album_id = Album.album_id AND at.format_name = 'webp'))
    OR
    (NOT EXISTS (SELECT * FROM AlbumThumbnail at WHERE at.album_id = Album.album_id AND at.format_name = 'avif'));
    "#
    )
        .load(conn).wrap_err("error querying for Albums with missing thumbnails")?;
    Ok(rows.into_iter().map(|r| AlbumId(r.album_id)).collect())
}
