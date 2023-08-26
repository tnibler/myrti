use crate::model::{
    repository::db_entity::DbFailedThumbnailJob, AssetId, FailedThumbnailJob, FailedThumbnailJobId,
};
use eyre::{Context, Result};
use tracing::{instrument, Instrument};

use super::pool::DbPool;

#[instrument(skip(pool))]
pub async fn insert_failed_thumbnail_job(
    pool: &DbPool,
    j: &FailedThumbnailJob,
) -> Result<FailedThumbnailJobId> {
    let db_value: DbFailedThumbnailJob = j.try_into()?;
    let result = sqlx::query!(
        r#"
INSERT INTO FailedThumbnailJob VALUES (?, ?, ?);
    "#,
        db_value.asset_id,
        db_value.file_hash,
        db_value.date
    )
    .execute(pool)
    .in_current_span()
    .await
    .wrap_err("could not insert into table FailedThumbnailJob")?;
    Ok(FailedThumbnailJobId(result.last_insert_rowid()))
}

#[instrument(skip(pool))]
pub async fn get_failed_thumbnail_job_for_asset(
    pool: &DbPool,
    asset_id: AssetId,
) -> Result<Option<FailedThumbnailJob>> {
    sqlx::query_as!(
        DbFailedThumbnailJob,
        r#"
SELECT
asset_id,
file_hash,
date
FROM FailedThumbnailJob
WHERE asset_id = ?;
    "#,
        asset_id
    )
    .fetch_optional(pool)
    .in_current_span()
    .await
    .wrap_err("could not query table FailedThumbnailJob")?
    .map(|j| (&j).try_into())
    .transpose()
}
