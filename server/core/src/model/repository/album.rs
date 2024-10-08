use std::borrow::Cow;

use chrono::Utc;
use diesel::{
    prelude::*,
    query_builder::{QueryBuilder, QueryFragment},
    sqlite::SqliteQueryBuilder,
};
use eyre::{eyre, Context, Result};
use tracing::instrument;

use crate::model::{
    self,
    repository::db_entity::{DbAlbum, DbAlbumWithItemCount, DbAsset, DbInsertAlbum},
    util::datetime_to_db_repr,
    Album, AlbumId, AlbumItem, AlbumItemId, AlbumItemType, Asset, AssetId,
};

use super::{db::DbConn, db_entity::DbAlbumItem, schema};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CreateAlbum {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// Get all albums ordered by changed_at (descending)
#[instrument(skip(conn), level = "trace")]
pub fn get_all_albums_with_asset_count(conn: &mut DbConn) -> Result<Vec<(Album, i64)>> {
    let mut qb = SqliteQueryBuilder::new();
    qb.push_sql("SELECT ");
    DbAlbum::as_select().to_sql(&mut qb, &diesel::sqlite::Sqlite)?;
    qb.push_sql(
        r#"
    , IFNULL(item_count, 0) as item_count 
    FROM Album
    LEFT JOIN 
        (
        SELECT album_id, COUNT(AlbumItem.album_item_id) as item_count
        FROM AlbumItem
        GROUP BY album_id
        ) g
    ON Album.album_id = g.album_id
    ORDER BY Album.changed_at DESC;
    "#,
    );
    let db_albums: Vec<DbAlbumWithItemCount> = diesel::sql_query(qb.finish()).load(conn)?;
    db_albums
        .into_iter()
        .map(|a| a.album.try_into().map(|album| (album, a.item_count)))
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
    use schema::{Album, AlbumItem};
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
        let _album_item_ids = assets
            .iter()
            .enumerate()
            .map(|(idx, asset_id)| {
                let album_item_id: i64 = diesel::insert_into(AlbumItem::table)
                    .values((
                        AlbumItem::album_id.eq(album_id.0),
                        AlbumItem::ty.eq(1),
                        AlbumItem::asset_id.eq(Some(asset_id.0)),
                        AlbumItem::text.eq(Option::<String>::None),
                        AlbumItem::idx.eq(i32::try_from(idx)?),
                    ))
                    .returning(AlbumItem::album_item_id)
                    .get_result(conn)?;
                Ok(AlbumItemId(album_item_id))
            })
            .collect::<Result<Vec<_>>>()
            .wrap_err("error inserting one or more AlbumItem")?;
        Ok::<AlbumId, eyre::Report>(album_id)
    })?;
    Ok(album_id)
}

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct AlbumItemRow {
    #[diesel(embed)]
    pub album_item: DbAlbumItem,
    #[diesel(embed)]
    pub asset: Option<DbAsset>,
}

#[instrument(skip(conn))]
pub fn get_items_in_album(conn: &mut DbConn, album_id: AlbumId) -> Result<Vec<AlbumItem>> {
    use schema::{AlbumItem, Asset};
    let rows: Vec<AlbumItemRow> = AlbumItem::table
        .left_join(Asset::table)
        .filter(AlbumItem::album_id.eq(album_id.0))
        .order_by(AlbumItem::idx)
        .select(AlbumItemRow::as_select())
        .load(conn)
        .wrap_err("error querying for album items")?;
    let items = rows
        .into_iter()
        .map(|row| match (row.asset, row.album_item.text) {
            (Some(asset), None) => {
                assert!(row.album_item.ty == 1);
                let asset = asset.try_into()?;
                Ok(model::AlbumItem {
                    id: AlbumItemId(row.album_item.album_item_id),
                    item: AlbumItemType::Asset(asset),
                })
            }
            (None, Some(text)) => {
                assert!(row.album_item.ty == 2);
                Ok(model::AlbumItem {
                    id: AlbumItemId(row.album_item.album_item_id),
                    item: AlbumItemType::Text(text),
                })
            }
            (asset, text) => {
                tracing::error!(album_item_id=?row.album_item.album_item_id, ?asset, ?text, "Invalid result row");
                Err(eyre!(
                    "Invalid result row: asset={}, text={}",
                    asset.is_some(),
                    text.is_some()
                ))
            }
        })
        .collect::<Result<Vec<_>>>()
        .wrap_err("error getting items in album")?;
    Ok(items)
}

/// Get assets in album ordered by the index of their AlbumItem index
#[instrument(skip(conn))]
pub fn get_assets_in_album(
    conn: &mut DbConn,
    album_id: AlbumId,
    limit: Option<i64>,
) -> Result<Vec<Asset>> {
    use schema::{AlbumItem, Asset};
    let query = AlbumItem::table
        .filter(AlbumItem::album_id.eq(album_id.0).and(AlbumItem::ty.eq(1)))
        .inner_join(Asset::table)
        .order_by(AlbumItem::idx)
        .select(DbAsset::as_select())
        .into_boxed();
    let db_assets: Vec<DbAsset> = if let Some(limit) = limit {
        query.limit(limit)
    } else {
        query
    }
    .load(conn)
    .wrap_err("error querying for Assets in Album")?;
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
    use schema::{Album, AlbumItem};
    conn.transaction(|conn| {
        let last_index: Option<i32> = AlbumItem::table
            .filter(AlbumItem::album_id.eq(album_id.0))
            .select(max(AlbumItem::idx))
            .get_result(conn)?;
        let first_insert_index = last_index.map(|last| last + 1).unwrap_or(0);
        let _album_item_ids = asset_ids
            .iter()
            .zip(first_insert_index..)
            .map(|(asset_id, idx)| {
                let album_item_id: i64 = diesel::insert_into(AlbumItem::table)
                    .values((
                        AlbumItem::album_id.eq(album_id.0),
                        AlbumItem::ty.eq(1),
                        AlbumItem::asset_id.eq(Some(asset_id.0)),
                        AlbumItem::text.eq(Option::<String>::None),
                        AlbumItem::idx.eq(idx),
                    ))
                    .returning(AlbumItem::album_item_id)
                    .get_result(conn)?;
                Ok(AlbumItemId(album_item_id))
            })
            .collect::<Result<Vec<_>>>()?;
        let now = datetime_to_db_repr(&Utc::now());
        diesel::update(Album::table)
            .set(Album::changed_at.eq(now))
            .execute(conn)?;
        Ok(())
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddItemToAlbum {
    Asset(AssetId),
    Text(String),
}

#[instrument(skip(conn))]
pub fn append_items_to_album(
    conn: &mut DbConn,
    album_id: AlbumId,
    items: &[AddItemToAlbum],
) -> Result<()> {
    use diesel::dsl::max;
    use schema::{Album, AlbumItem};
    conn.immediate_transaction(|conn| {
        let last_index: Option<i32> = AlbumItem::table
            .filter(AlbumItem::album_id.eq(album_id.0))
            .select(max(AlbumItem::idx))
            .get_result(conn)?;
        let first_insert_index = last_index.map(|last| last + 1).unwrap_or(0);
        let _album_item_ids = items
            .iter()
            .zip(first_insert_index..)
            .map(|(item, idx)| {
                let insert_row = match item {
                    AddItemToAlbum::Asset(asset_id) => (
                        AlbumItem::album_id.eq(album_id.0),
                        AlbumItem::ty.eq(1),
                        AlbumItem::asset_id.eq(Some(asset_id.0)),
                        AlbumItem::text.eq(Option::<&str>::None),
                        AlbumItem::idx.eq(idx),
                    ),
                    AddItemToAlbum::Text(text) => (
                        AlbumItem::album_id.eq(album_id.0),
                        AlbumItem::ty.eq(2),
                        AlbumItem::asset_id.eq(Option::<i64>::None),
                        AlbumItem::text.eq(Some(text.as_str())),
                        AlbumItem::idx.eq(idx),
                    ),
                };
                let album_item_id: i64 = diesel::insert_into(AlbumItem::table)
                    .values(insert_row)
                    .returning(AlbumItem::album_item_id)
                    .get_result(conn)?;
                Ok(AlbumItemId(album_item_id))
            })
            .collect::<Result<Vec<_>>>()?;
        let now = datetime_to_db_repr(&Utc::now());
        diesel::update(Album::table)
            .set(Album::changed_at.eq(now))
            .execute(conn)?;
        Ok(())
    })
}

#[instrument(skip(conn))]
pub fn remove_items_from_album(
    conn: &mut DbConn,
    album_id: AlbumId,
    item_ids: &[AlbumItemId],
) -> Result<()> {
    if item_ids.is_empty() {
        return Ok(());
    }
    let mut delete_qb = diesel::sqlite::SqliteQueryBuilder::new();
    delete_qb.push_sql(
        r#"
            DELETE FROM AlbumItem 
            WHERE AlbumItem.album_id = $1
            AND AlbumItem.album_item_id IN (
        "#,
    );
    for (idx, _) in item_ids.iter().enumerate() {
        delete_qb.push_bind_param();
        if idx != item_ids.len() - 1 {
            delete_qb.push_sql(", ");
        }
    }
    delete_qb.push_sql(
        r#"
    );
    "#,
    );
    let delete_query = {
        let mut q = diesel::sql_query(delete_qb.finish())
            .bind::<diesel::sql_types::BigInt, _>(album_id.0)
            .into_boxed();
        for item_id in item_ids {
            q = q.bind::<diesel::sql_types::BigInt, _>(item_id.0);
        }
        q
    };
    conn.immediate_transaction(|conn| {
        // delete rows
        let affected_rows = delete_query.execute(conn).wrap_err("error deleting rows from table AlbumItem")?;
        debug_assert!(affected_rows == item_ids.len());
        // reset AlbumItem.idx column to 0..n
        diesel::sql_query(r#"
            WITH new_idxs AS (
                SELECT AlbumItem.album_item_id, (DENSE_RANK() OVER (ORDER BY idx) - 1) AS new_idx FROM AlbumItem WHERE 
                AlbumItem.album_id = $1
                ORDER BY AlbumItem.idx
            )
            UPDATE AlbumItem
            SET 
            idx = (SELECT new_idx FROM new_idxs WHERE new_idxs.album_item_id = AlbumItem.album_item_id)
            WHERE AlbumItem.album_id = $1;
        "#)
            .bind::<diesel::sql_types::BigInt, _>(album_id.0)
            .execute(conn).wrap_err("error assigning new AlbumItem indices")?;
        Ok::<_, eyre::Report>(())
        })?;
    Ok(())
}
