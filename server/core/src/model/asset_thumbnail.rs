use super::{AssetId, AssetThumbnailId, Size, ThumbnailFormat, ThumbnailType};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetThumbnail {
    pub id: AssetThumbnailId,
    pub asset_id: AssetId,
    pub ty: ThumbnailType,
    pub size: Size,
    pub format: ThumbnailFormat,
}
