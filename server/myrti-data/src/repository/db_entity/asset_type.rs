use crate::model::AssetType;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Copy)]
#[repr(i32)]
pub enum DbAssetType {
    Image = 1,
    Video = 2,
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
