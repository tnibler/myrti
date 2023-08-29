use crate::model::{
    repository::db_entity::{DbAudioRepresentation, DbVideoRepresentation},
    AssetId, AudioRepresentation, AudioRepresentationId, VideoRepresentation,
    VideoRepresentationId,
};

use eyre::{Context, Result};
use sqlx::SqliteConnection;
use tracing::instrument;

use super::pool::DbPool;

#[instrument(skip(pool))]
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
media_info_key
FROM VideoRepresentation 
WHERE asset_id=?;
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
INSERT INTO VideoRepresentation VALUES(NULL, ?, ?, ?, ?, ?, ?, ?);
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

#[instrument(skip(conn))]
pub async fn insert_audio_representation(
    conn: &mut SqliteConnection,
    repr: &AudioRepresentation,
) -> Result<AudioRepresentationId> {
    assert!(repr.id.0 == 0);
    let db_val: DbAudioRepresentation = repr.try_into()?;
    let result = sqlx::query!(
        r#"
INSERT INTO AudioRepresentation VALUES(NULL, ?, ?, ?, ?);
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
