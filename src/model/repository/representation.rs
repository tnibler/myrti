use crate::model::{
    repository::db_entity::{DbAudioRepresentation, DbVideoRepresentation},
    AudioRepresentation, AudioRepresentationId, VideoRepresentation, VideoRepresentationId,
};

use eyre::{Context, Result};
use sqlx::SqliteConnection;
use tracing::instrument;

#[instrument(skip(conn))]
pub async fn insert_video_representation(
    conn: &mut SqliteConnection,
    repr: VideoRepresentation,
) -> Result<VideoRepresentationId> {
    assert!(repr.id.0 == 0);
    let db_val: DbVideoRepresentation = repr.try_into()?;
    let result = sqlx::query!(
        r#"
INSERT INTO VideoRepresentation VALUES(NULL, ?, ?, ?, ?, ?, ?);
    "#,
        db_val.asset_id,
        db_val.codec_name,
        db_val.width,
        db_val.height,
        db_val.bitrate,
        db_val.path_in_resource_dir
    )
    .execute(conn)
    .await
    .wrap_err("could not insert into table VideoRepresentation")?;
    Ok(VideoRepresentationId(result.last_insert_rowid()))
}

#[instrument(skip(conn))]
pub async fn insert_audio_representation(
    conn: &mut SqliteConnection,
    repr: AudioRepresentation,
) -> Result<AudioRepresentationId> {
    assert!(repr.id.0 == 0);
    let db_val: DbAudioRepresentation = repr.try_into()?;
    let result = sqlx::query!(
        r#"
INSERT INTO AudioRepresentation VALUES(NULL, ?, ?);
    "#,
        db_val.asset_id,
        db_val.path_in_resource_dir
    )
    .execute(conn)
    .await
    .wrap_err("could not insert into table AudioRepresentation")?;
    Ok(AudioRepresentationId(result.last_insert_rowid()))
}
