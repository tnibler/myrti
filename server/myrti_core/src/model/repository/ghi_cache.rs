use diesel::prelude::*;
use eyre::{Context, Result, eyre};

use crate::model::{
    FileId,
    repository::{db::DbConn, schema},
};

#[derive(Insertable)]
#[diesel(table_name = schema::GhiSegmentCache)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct InsertSegmentRow<'a> {
    file_id: i64,
    file_name: &'a str,
    status: i32,
}

pub fn insert_pending_segment(conn: &mut DbConn, file_id: FileId, file_name: &str) -> Result<()> {
    use schema::GhiSegmentCache;
    conn.immediate_transaction(|conn| {
        diesel::insert_into(GhiSegmentCache::table)
            .values(InsertSegmentRow {
                file_id: file_id.0,
                file_name,
                status: 0,
            })
            .execute(conn)
            .wrap_err("error inserting into table GhiSegmentCache")?;
        Ok(())
    })
}

pub fn finalize_segment(
    conn: &mut DbConn,
    file_id: FileId,
    file_name: &str,
    size: i64,
) -> Result<()> {
    use schema::GhiSegmentCache;
    conn.immediate_transaction(|conn| {
        let n_affected = diesel::update(GhiSegmentCache::table)
            .filter(
                GhiSegmentCache::file_id
                    .eq(file_id.0)
                    .and(GhiSegmentCache::file_name.eq(file_name)),
            )
            .set((
                GhiSegmentCache::status.eq(1),
                GhiSegmentCache::size.eq(size),
            ))
            .execute(conn)?;
        if n_affected != 1 {
            return Err(eyre!(
                "error finalizing GhiSegmentCache row: expected 1 modified row but got {}",
                n_affected
            ));
        }
        Ok(())
    })
}

pub fn update_accessed_time(conn: &mut DbConn, file_id: FileId, file_name: &str) -> Result<()> {
    use schema::GhiSegmentCache;
    conn.immediate_transaction(|conn| {
        let n_affected = diesel::update(GhiSegmentCache::table)
            .filter(
                GhiSegmentCache::file_id
                    .eq(file_id.0)
                    .and(GhiSegmentCache::file_name.eq(file_name)),
            )
            // trigger will set it to the current timestamp
            .set((GhiSegmentCache::accessed_at.eq(1),))
            .execute(conn)?;
        if n_affected != 1 {
            return Err(eyre!(
                "error updating GhiSegmentCache accessed time: expected 1 modified row but got {}",
                n_affected
            ));
        }
        Ok(())
    })
}
