use super::{AssetThumbnailId, FileId, Size, ThumbnailFormat, ThumbnailType};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetThumbnail {
    pub id: AssetThumbnailId,
    pub file_id: FileId,
    pub ty: ThumbnailType,
    pub size: Size,
    pub format: ThumbnailFormat,
}
