use eyre::{eyre, Context, Result};
use sqlx::SqliteConnection;
use tracing::{instrument, Instrument};

use crate::model::{
    repository::db_entity::{DbAudioRepresentation, DbImageRepresentation, DbVideoRepresentation},
    AssetId, AudioRepresentation, AudioRepresentationId, ImageRepresentation,
    ImageRepresentationId, VideoRepresentation, VideoRepresentationId,
};

use super::pool::DbPool;

#[instrument(skip(pool))]
/// get all valid (not reserved) representations for a video asset
pub async fn get_video_representations(
    pool: &DbPool,
    asset_id: AssetId,
) -> Result<Vec<VideoRepresentation>> {
    sqlx::query_as!(
        DbVideoRepresentation,
        r#"
SELECT
id,
asset_id,
codec_name,
width,
height,
bitrate,
file_key,
media_info_key,
is_preallocated_dummy
FROM VideoRepresentation 
WHERE asset_id=?
AND is_preallocated_dummy=0;
    "#,
        asset_id
    )
    .fetch_all(pool)
    .await
    .wrap_err("could not query table VideoRepresentation")?
    .into_iter()
    .map(|r| r.try_into())
    .collect::<Result<Vec<_>>>()
}

#[instrument(skip(pool))]
pub async fn get_audio_representation(
    pool: &DbPool,
    asset_id: AssetId,
) -> Result<Option<AudioRepresentation>> {
    sqlx::query_as!(
        DbAudioRepresentation,
        r#"
SELECT
id,
asset_id,
codec_name,
file_key,
media_info_key
FROM AudioRepresentation 
WHERE asset_id=?;
    "#,
        asset_id
    )
    .fetch_optional(pool)
    .await
    .wrap_err("could not query table AudioRepresentation")?
    .map(|r| r.try_into())
    .transpose()
}

#[instrument(skip(conn))]
pub async fn insert_video_representation(
    conn: &mut SqliteConnection,
    repr: &VideoRepresentation,
) -> Result<VideoRepresentationId> {
    assert!(repr.id.0 == 0);
    let db_val: DbVideoRepresentation = repr.try_into()?;
    let result = sqlx::query!(
        r#"
INSERT INTO VideoRepresentation
(id, asset_id, codec_name, width, height, bitrate, file_key, media_info_key, is_preallocated_dummy)
VALUES(NULL, ?, ?, ?, ?, ?, ?, ?, 0);
    "#,
        db_val.asset_id,
        db_val.codec_name,
        db_val.width,
        db_val.height,
        db_val.bitrate,
        db_val.file_key,
        db_val.media_info_key
    )
    .execute(conn)
    .await
    .wrap_err("could not insert into table VideoRepresentation")?;
    Ok(VideoRepresentationId(result.last_insert_rowid()))
}

#[instrument(skip(pool), level = "debug")]
pub async fn reserve_video_representation(
    pool: &DbPool,
    asset_id: AssetId,
    codec_name: &str,
) -> Result<VideoRepresentationId> {
    let result = sqlx::query!(
        r#"
INSERT INTO VideoRepresentation(id, asset_id, codec_name, width, height, bitrate, file_key, media_info_key, is_preallocated_dummy)
VALUES (NULL, ?, ?, NULL, NULL, NULL, NULL, NULL, 1);
    "#,
        asset_id,
        codec_name
    )
        .execute(pool)
        .await
        .wrap_err("could not insert dummy row into table VideoRepresentation")?;
    Ok(VideoRepresentationId(result.last_insert_rowid()))
}

#[instrument(skip(pool), level = "debug")]
pub async fn reserve_image_representation(
    pool: &DbPool,
    asset_id: AssetId,
    format_name: &str,
) -> Result<ImageRepresentationId> {
    let result = sqlx::query!(
        r#"
INSERT INTO ImageRepresentation(id, asset_id, format_name, width, height, file_size, file_key, is_preallocated_dummy)
VALUES (NULL, ?, ?, NULL, NULL, NULL, NULL, 1);
    "#,
        asset_id,
        format_name
    )
        .execute(pool)
        .await
        .wrap_err("could not insert dummy row into table VideoRepresentation")?;
    Ok(ImageRepresentationId(result.last_insert_rowid()))
}

#[instrument(skip(conn), level = "debug")]
pub async fn update_dummy_video_representation(
    conn: &mut SqliteConnection,
    video_representation: &VideoRepresentation,
) -> Result<()> {
    let db_repr: DbVideoRepresentation = video_representation.try_into()?;
    let result = sqlx::query!(
        r#"
UPDATE VideoRepresentation
SET width=?, height=?, bitrate=?, file_key=?, media_info_key=?, is_preallocated_dummy=0
WHERE id = ?
AND asset_id = ?
AND is_preallocated_dummy != 0;
    "#,
        db_repr.width,
        db_repr.height,
        db_repr.bitrate,
        db_repr.file_key,
        db_repr.media_info_key,
        // make sure the row id and referenced asset_id still match the values the row was reserved_or
        db_repr.id,
        db_repr.asset_id
    )
    .execute(conn)
    .await
    .wrap_err("could not update dummy row in table VideoRepresentation")?;
    // id is invalid, or the ids and asset_id got mixed up or the row is not a dummy
    if result.rows_affected() != 1 {
        return Err(eyre!("no matching dummy row found to update"));
    }
    Ok(())
}

#[instrument(skip(pool), level = "debug")]
pub async fn update_dummy_image_representation(
    pool: &DbPool,
    image_representation: &ImageRepresentation,
) -> Result<()> {
    let db_repr: DbImageRepresentation = image_representation.try_into()?;
    let result = sqlx::query!(
        r#"
UPDATE ImageRepresentation
SET width=?, height=?, file_size=?, file_key=?, is_preallocated_dummy=0
WHERE id = ?
AND asset_id = ?
AND is_preallocated_dummy != 0;
    "#,
        db_repr.width,
        db_repr.height,
        db_repr.file_size,
        db_repr.file_key,
        // make sure the row id and referenced asset_id still match the values the row was reserved_or
        db_repr.id,
        db_repr.asset_id
    )
    .execute(pool)
    .await
    .wrap_err("could not update dummy row in table ImageRepresentation")?;
    // id is invalid, or the ids and asset_id got mixed up or the row is not a dummy
    if result.rows_affected() != 1 {
        return Err(eyre!("no matching dummy row found to update"));
    }
    Ok(())
}

#[instrument(skip(conn))]
pub async fn insert_audio_representation(
    conn: &mut SqliteConnection,
    repr: &AudioRepresentation,
) -> Result<AudioRepresentationId> {
    assert!(repr.id.0 == 0);
    let db_val: DbAudioRepresentation = repr.try_into()?;
    let result = sqlx::query!(
        r#"
INSERT INTO AudioRepresentation
(id, asset_id, codec_name, file_key, media_info_key)
VALUES(NULL, ?, ?, ?, ?);
    "#,
        db_val.asset_id,
        db_val.codec_name,
        db_val.file_key,
        db_val.media_info_key
    )
    .execute(conn)
    .await
    .wrap_err("could not insert into table AudioRepresentation")?;
    Ok(AudioRepresentationId(result.last_insert_rowid()))
}

#[tracing::instrument(skip(pool), level = "debug")]
pub async fn insert_image_representation(
    pool: &DbPool,
    repr: &ImageRepresentation,
) -> Result<ImageRepresentationId> {
    let result = sqlx::query!(
        r#"
INSERT INTO ImageRepresentation
(id, asset_id, format_name, width, height, file_size, file_key, is_preallocated_dummy)
VALUES (NULL, ?, ?, ?, ?, ?, ?, 0);
    "#,
        repr.asset_id,
        repr.format_name,
        repr.width,
        repr.height,
        repr.file_size,
        repr.file_key
    )
    .execute(pool)
    .in_current_span()
    .await
    .wrap_err("could not insert into table ImageRepresentation")?;
    Ok(ImageRepresentationId(result.last_insert_rowid()))
}

#[tracing::instrument(skip(pool), level = "trace")]
pub async fn get_image_representation(
    pool: &DbPool,
    id: ImageRepresentationId,
) -> Result<ImageRepresentation> {
    sqlx::query_as!(
        DbImageRepresentation,
        r#"
SELECT 
id,
asset_id,
format_name,
width,
height,
file_size,
file_key,
is_preallocated_dummy
FROM ImageRepresentation
WHERE id = ?;
    "#,
        id
    )
    .fetch_one(pool)
    .in_current_span()
    .await
    .wrap_err("could not query table ImageRepresentation")?
    .try_into()
}

#[tracing::instrument(skip(pool), level = "debug")]
pub async fn get_image_representations(
    pool: &DbPool,
    asset_id: AssetId,
) -> Result<Vec<ImageRepresentation>> {
    sqlx::query_as!(
        DbImageRepresentation,
        r#"
SELECT 
id,
asset_id,
format_name,
width,
height,
file_size,
file_key,
is_preallocated_dummy
FROM ImageRepresentation
WHERE asset_id = ?;
    "#,
        asset_id
    )
    .fetch_all(pool)
    .in_current_span()
    .await
    .wrap_err("could not query table ImageRepresentation")?
    .into_iter()
    .map(|db_image_repr| db_image_repr.try_into())
    .collect::<Result<Vec<_>>>()
}
