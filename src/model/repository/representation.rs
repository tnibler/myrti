use std::ops::DerefMut;

use crate::{
    core::NewResourceFile,
    model::{repository::resource_file, ResourceFile, VideoRepresentation, VideoRepresentationId},
};

use super::pool::DbPool;
use eyre::{Context, Result};
use sqlx::{Executor, Sqlite, SqliteConnection, SqliteExecutor, Transaction};
use tracing::{field::debug, instrument};

#[instrument(name = "Insert VideoRepresentation", skip(pool,))]
pub async fn insert_video_representation(
    pool: &DbPool,
    repr: VideoRepresentation,
    resource_file: NewResourceFile,
) -> Result<VideoRepresentationId> {
    debug_assert!(repr.id.0 == 0);
    let mut tx = pool
        .begin()
        .await
        .wrap_err("could not begin db transaction")?;
    let resource_file_id = resource_file::insert_new_resource_file(&mut tx, resource_file).await?;
    let result = sqlx::query!(
        r#"
INSERT INTO VideoRepresentation VALUES(NULL, ?, ?, ?, ?, ?, ?);
    "#,
        repr.asset_id,
        repr.codec_name,
        repr.width,
        repr.height,
        repr.bitrate,
        resource_file_id
    )
    .execute(tx.deref_mut())
    .await?;
    tx.commit()
        .await
        .wrap_err("could not commit db transaction")?;
    Ok(VideoRepresentationId(result.last_insert_rowid()))
}
