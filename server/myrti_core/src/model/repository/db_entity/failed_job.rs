use diesel::{Queryable, Selectable};

use crate::model::{
    util::{datetime_from_db_repr, hash_vec8_to_u64},
    AssetId, FailedThumbnailJob,
};

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::FailedThumbnailJob)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbFailedThumbnailJob {
    pub asset_id: i64,
    pub file_hash: Vec<u8>,
    pub date: i64,
}

impl TryFrom<DbFailedThumbnailJob> for FailedThumbnailJob {
    type Error = eyre::Report;

    fn try_from(value: DbFailedThumbnailJob) -> Result<Self, Self::Error> {
        Ok(FailedThumbnailJob {
            asset_id: AssetId(value.asset_id),
            file_hash: hash_vec8_to_u64(value.file_hash)?,
            date: datetime_from_db_repr(value.date)?,
        })
    }
}
