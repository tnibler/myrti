use crate::model::{AssetId, ResourceFileId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbVideoInfo {
    pub asset_id: AssetId,
    pub dash_resource_dir: Option<ResourceFileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbImageInfo {
    pub asset_id: AssetId,
}
