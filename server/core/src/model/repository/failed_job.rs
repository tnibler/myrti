use crate::model::util::{datetime_to_db_repr, hash_u64_to_vec8};
use crate::model::{repository::db_entity::DbFailedThumbnailJob, AssetId, FailedThumbnailJob};
use diesel::prelude::*;
use eyre::Result;
use tracing::instrument;

use super::db::DbConn;
use super::schema;

#[instrument(skip(conn), level = "trace")]
pub fn insert_failed_thumbnail_job(conn: &mut DbConn, j: &FailedThumbnailJob) -> Result<()> {
    use schema::FailedThumbnailJob;
    diesel::insert_into(FailedThumbnailJob::table)
        .values((
            FailedThumbnailJob::asset_id.eq(j.asset_id.0),
            FailedThumbnailJob::file_hash.eq(hash_u64_to_vec8(j.file_hash)),
            FailedThumbnailJob::date.eq(datetime_to_db_repr(&j.date)),
        ))
        .execute(conn)?;
    Ok(())
}

#[instrument(skip(conn), level = "trace")]
pub fn get_failed_thumbnail_job_for_asset(
    conn: &mut DbConn,
    asset_id: AssetId,
) -> Result<Option<FailedThumbnailJob>> {
    use schema::FailedThumbnailJob;
    let db_ftj: Option<DbFailedThumbnailJob> = FailedThumbnailJob::table
        .filter(FailedThumbnailJob::asset_id.eq(asset_id.0))
        .first(conn)
        .optional()?;
    db_ftj.map(|db_ftj| db_ftj.try_into()).transpose()
}
