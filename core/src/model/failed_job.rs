use chrono::{DateTime, Utc};
use eyre::Result;

use super::{
    repository::db_entity::DbFailedThumbnailJob,
    util::{datetime_from_db_repr, datetime_to_db_repr, hash_u64_to_vec8, hash_vec8_to_u64},
    AssetId,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FailedThumbnailJob {
    pub asset_id: AssetId,
    pub file_hash: u64,
    pub date: DateTime<Utc>,
}

pub enum FailedVideoPackagingJob {
    Transcoding,
    PackageOriginal,
}

impl TryFrom<&DbFailedThumbnailJob> for FailedThumbnailJob {
    type Error = eyre::Report;

    fn try_from(value: &DbFailedThumbnailJob) -> Result<Self, Self::Error> {
        Ok(FailedThumbnailJob {
            asset_id: value.asset_id,
            file_hash: hash_vec8_to_u64(&value.file_hash)?,
            date: datetime_from_db_repr(value.date)?,
        })
    }
}

impl TryFrom<&FailedThumbnailJob> for DbFailedThumbnailJob {
    type Error = eyre::Report;

    fn try_from(value: &FailedThumbnailJob) -> std::result::Result<Self, Self::Error> {
        Ok(DbFailedThumbnailJob {
            asset_id: value.asset_id,
            file_hash: hash_u64_to_vec8(value.file_hash),
            date: datetime_to_db_repr(&value.date),
        })
    }
}
