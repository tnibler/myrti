use crate::model::AssetId;

#[derive(Debug, Clone, sqlx::Type)]
pub struct DbFailedThumbnailJob {
    pub asset_id: AssetId,
    pub file_hash: Vec<u8>,
    pub date: String,
}
