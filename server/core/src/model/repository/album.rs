use std::borrow::Cow;

use chrono::Utc;
use diesel::prelude::*;
use eyre::{Context, Result};
use tracing::instrument;

use crate::model::{
    self,
    repository::db_entity::{DbAlbum, DbAlbumWithAssetCount, DbAsset, DbInsertAlbum},
    util::datetime_to_db_repr,
    Album, AlbumEntryId, AlbumId, Asset, AssetId,
};

use super::{db::DbConn, schema};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CreateAlbum {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// Get all albums ordered by changed_at (descending)
#[instrument(skip(conn), level = "trace")]
pub fn get_all_albums_with_asset_count(conn: &mut DbConn) -> Result<Vec<(Album, i64)>> {
    use diesel::dsl::count;
    use schema::{Album, AlbumEntry};
    let db_albums: Vec<DbAlbumWithAssetCount> = Album::table
        .inner_join(AlbumEntry::table)
        .group_by(Album::album_id)
        .select((DbAlbum::as_select(), count(AlbumEntry::album_entry_id)))
        .load(conn)?;
    db_albums
        .into_iter()
        .map(|a| a.album.try_into().map(|album| (album, a.asset_count)))
        .collect::<Result<Vec<(model::Album, i64)>>>()
}

#[instrument(skip(conn), level = "trace")]
pub fn get_album(conn: &mut DbConn, album_id: AlbumId) -> Result<Album> {
    use schema::Album;
    let db_album: DbAlbum = Album::table.find(album_id.0).first(conn)?;
    db_album.try_into()
}

#[instrument(err(Debug), skip(conn), level = "trace")]
pub fn create_album(
    conn: &mut DbConn,
    create_album: CreateAlbum,
    assets: &[AssetId],
) -> Result<AlbumId> {
    use schema::{Album, AlbumEntry};
    let now = datetime_to_db_repr(&Utc::now());
    let album_id: AlbumId = conn.transaction(|conn| {
        let album_id = diesel::insert_into(Album::table)
            .values(DbInsertAlbum {
                album_id: None,
                name: create_album.name.map(Cow::Owned),
                description: create_album.description.map(Cow::Owned),
                created_at: now,
                changed_at: now,
            })
            .returning(Album::album_id)
            .get_result(conn)
            .map(AlbumId)
            .wrap_err("Error inserting Album")?;
        let album_entry_ids = assets
            .iter()
            .enumerate()
            .map(|(idx, asset_id)| {
                let album_entry_id: i64 = diesel::insert_into(AlbumEntry::table)
                    .values((
                        AlbumEntry::album_id.eq(album_id.0),
                        AlbumEntry::ty.eq(1),
                        AlbumEntry::asset_id.eq(Some(asset_id.0)),
                        AlbumEntry::text.eq(Option::<String>::None),
                        AlbumEntry::idx.eq(i32::try_from(idx)?),
                    ))
                    .returning(AlbumEntry::album_entry_id)
                    .get_result(conn)?;
                Ok(AlbumEntryId(album_entry_id))
            })
            .collect::<Result<Vec<_>>>()
            .wrap_err("error inserting one or more AlbumEntry")?;
        Ok::<AlbumId, eyre::Report>(album_id)
    })?;
    Ok(album_id)
}

/// Get assets in album ordered by the index of their AlbumEntry index
#[instrument(skip(conn), level = "trace")]
pub fn get_assets_in_album(conn: &mut DbConn, album_id: AlbumId) -> Result<Vec<Asset>> {
    use schema::{AlbumEntry, Asset};
    let db_assets: Vec<DbAsset> = AlbumEntry::table
        .filter(
            AlbumEntry::album_id
                .eq(album_id.0)
                .and(AlbumEntry::ty.eq(1)),
        )
        .inner_join(Asset::table)
        .order_by(AlbumEntry::idx)
        .select(DbAsset::as_select())
        .load(conn)?;
    db_assets
        .into_iter()
        .map(|db_asset| db_asset.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn), level = "trace")]
pub fn append_assets_to_album(
    conn: &mut DbConn,
    album_id: AlbumId,
    asset_ids: &[AssetId],
) -> Result<()> {
    use diesel::dsl::max;
    use schema::{Album, AlbumEntry};
    conn.transaction(|conn| {
        let last_index: Option<i32> = AlbumEntry::table
            .filter(AlbumEntry::album_id.eq(album_id.0))
            .select(max(AlbumEntry::idx))
            .get_result(conn)?;
        let first_insert_index = last_index.map(|last| last + 1).unwrap_or(0);
        let _album_entry_ids = asset_ids
            .iter()
            .zip(first_insert_index..)
            .map(|(asset_id, idx)| {
                let album_entry_id: i64 = diesel::insert_into(AlbumEntry::table)
                    .values((
                        AlbumEntry::album_id.eq(album_id.0),
                        AlbumEntry::ty.eq(1),
                        AlbumEntry::asset_id.eq(Some(asset_id.0)),
                        AlbumEntry::text.eq(Option::<String>::None),
                        AlbumEntry::idx.eq(idx),
                    ))
                    .returning(AlbumEntry::album_entry_id)
                    .get_result(conn)?;
                Ok(AlbumEntryId(album_entry_id))
            })
            .collect::<Result<Vec<_>>>()?;
        let now = datetime_to_db_repr(&Utc::now());
        diesel::update(Album::table)
            .set(Album::changed_at.eq(now))
            .execute(conn)?;
        Ok(())
    })
}
