use eyre::{Context, Result};

use crate::db::DbConn;

pub mod album;
pub mod album_thumbnail;
pub mod asset;
pub mod asset_root_dir;
pub mod asset_series;
pub mod config;
#[allow(dead_code)]
pub mod db_entity;
pub mod duplicate_asset;
pub mod ghi_cache;
pub mod representation;
#[allow(non_snake_case)]
mod schema;
pub mod timeline;
pub mod timeline_group;
pub(crate) mod util;

pub fn startup_delete_stale_rows(conn: &mut DbConn) -> Result<()> {
    use diesel::prelude::*;
    use schema::{AudioRepresentation, GhiSegmentCache, VideoRepresentation};

    let video_reprs_deleted = diesel::delete(
        VideoRepresentation::table.filter(VideoRepresentation::created_status.eq(0)),
    )
    .execute(conn)
    .wrap_err("error deleting stale VideoRepresentation rows")?;

    let audio_reprs_deleted = diesel::delete(
        AudioRepresentation::table.filter(AudioRepresentation::created_status.eq(0)),
    )
    .execute(conn)
    .wrap_err("error deleting stale AudioRepresentation rows")?;

    let ghi_cache_deleted =
        diesel::delete(GhiSegmentCache::table.filter(GhiSegmentCache::status.eq(0)))
            .execute(conn)
            .wrap_err("error deleting stale GhiSegmentCache rows")?;
    tracing::debug!(%video_reprs_deleted, %audio_reprs_deleted, %ghi_cache_deleted, "deleted stale rows");
    Ok(())
}
