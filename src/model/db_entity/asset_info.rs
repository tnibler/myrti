use crate::model::AssetId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbVideoInfo {
    pub asset_id: AssetId,
    pub dash_manifest_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbImageInfo {
    pub asset_id: AssetId,
}
