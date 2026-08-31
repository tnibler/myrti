use chrono::{DateTime, Utc};

use super::AssetId;

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
