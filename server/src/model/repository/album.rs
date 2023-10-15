use eyre::{Context, Result};
use tracing::Instrument;

use crate::model::{repository::db_entity::DbAsset, Album, AlbumEntryId, AlbumId, Asset, AssetId};

use super::pool::DbPool;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CreateAlbum {
    pub name: String,
    pub description: Option<String>,
}

pub async fn create_album(pool: &DbPool, create_album: CreateAlbum) -> Result<AlbumId> {
    let now = chrono::Utc::now();
    let album = Album {
        id: AlbumId(0),
        name: create_album.name,
        description: create_album.description,
        created_at: now,
        changed_at: now,
    };
    insert_album(pool, &album).await
}

pub async fn insert_album(pool: &DbPool, album: &Album) -> Result<AlbumId> {
    let created_at = album.created_at.timestamp();
    let changed_at = album.changed_at.timestamp();
    let result = sqlx::query!(
        r#"
INSERT INTO Album(id, name, description, created_at, changed_at)
VALUES
(NULL, ?, ?, ?, ?);
    "#,
        album.name,
        album.description,
        created_at,
        changed_at,
    )
    .execute(pool)
    .in_current_span()
    .await
    .wrap_err("could not insert into table Album")?;
    let id = AlbumId(result.last_insert_rowid());
    Ok(id)
}

pub async fn get_assets_in_album(album_id: AlbumId, pool: &DbPool) -> Result<Vec<Asset>> {
    sqlx::query_as!(
        DbAsset,
        r#"
SELECT 
Asset.id,
Asset.ty as "ty: _",
Asset.root_dir_id,
Asset.file_path,
Asset.file_type,
Asset.hash,
Asset.is_hidden,
Asset.added_at,
Asset.taken_date,
Asset.timezone_offset,
Asset.timezone_info as "timezone_info: _",
Asset.width,
Asset.height,
Asset.rotation_correction as "rotation_correction: _",
Asset.gps_latitude as "gps_latitude: _",
Asset.gps_longitude as "gps_longitude: _",
Asset.thumb_small_square_avif as "thumb_small_square_avif: _",
Asset.thumb_small_square_webp as "thumb_small_square_webp: _",
Asset.thumb_large_orig_avif as "thumb_large_orig_avif: _",
Asset.thumb_large_orig_webp as "thumb_large_orig_webp: _",
Asset.thumb_small_square_width,
Asset.thumb_small_square_height,
Asset.thumb_large_orig_width,
Asset.thumb_large_orig_height,
Asset.image_format_name,
Asset.video_codec_name,
Asset.video_bitrate,
Asset.audio_codec_name,
Asset.has_dash
FROM Asset INNER JOIN AlbumEntry
ON Asset.id = AlbumEntry.asset_id
WHERE AlbumEntry.album_id = ?
ORDER BY AlbumEntry.idx;
    "#,
        album_id
    )
    .fetch_all(pool)
    .await
    .wrap_err("could not query tables AlbumEntry, Asset")?
    .into_iter()
    .map(|db_asset| db_asset.try_into())
    .collect::<Result<Vec<Asset>>>()
}

pub async fn append_assets_to_album(
    pool: &DbPool,
    album_id: AlbumId,
    asset_ids: impl IntoIterator<Item = AssetId>,
) -> Result<()> {
    return Err(eyre::eyre!("some error"));

    let mut tx = pool.begin().await?;
    let last_index = sqlx::query!(
        r#"
SELECT MAX(AlbumEntry.idx) as max_index FROM AlbumEntry WHERE AlbumEntry.album_id = ?;
    "#,
        album_id
    )
    // we get one value, either index or null
    .fetch_one(tx.as_mut())
    .await
    .wrap_err("could not query table AlbumEntry")?
    .max_index;
    let first_insert_index = last_index.map_or(0, |last| last + 1);
    let insert_tuples = asset_ids.into_iter().zip(first_insert_index..);
    let mut query_builder = sqlx::QueryBuilder::new(
        r#"
INSERT INTO AlbumEntry(id, album_id, asset_id, idx)
    "#,
    );
    query_builder.push_values(insert_tuples, move |mut builder, (asset_id, index)| {
        builder.push_bind(None::<AlbumEntryId>);
        builder.push_bind(album_id);
        builder.push_bind(asset_id);
        builder.push_bind(index);
    });
    query_builder.push(r#";"#);
    query_builder
        .build()
        .execute(tx.as_mut())
        .await
        .wrap_err("could not insert into table AlbumEntry")?;

    // update changed_at field of album we just appended to
    let now = chrono::Utc::now().timestamp();
    sqlx::query!(
        r#"
UPDATE Album SET changed_at=? WHERE id = ?;
    "#,
        now,
        album_id
    )
    .execute(tx.as_mut())
    .await
    .wrap_err("could not update column Album.changed_at")?;
    tx.commit()
        .await
        .wrap_err("could not insert into table AlbumEntry")?;
    Ok(())
}
