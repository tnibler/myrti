use serde::Serialize;

use super::db_entity::DbAssetType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Copy)]
pub enum AssetType {
    Image,
    Video,
}

impl From<DbAssetType> for AssetType {
    fn from(value: DbAssetType) -> Self {
        match value {
            DbAssetType::Image => AssetType::Image,
            DbAssetType::Video => AssetType::Video,
        }
    }
}

impl From<AssetType> for DbAssetType {
    fn from(value: AssetType) -> Self {
        match value {
            AssetType::Image => Self::Image,
            AssetType::Video => Self::Video,
        }
    }
}
