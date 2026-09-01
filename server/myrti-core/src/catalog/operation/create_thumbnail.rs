use myrti_data::model::{FileId, ThumbnailFormat, ThumbnailType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetThumbnail {
    pub file_id: FileId,
    pub formats: Vec<ThumbnailFormat>,
    pub thumbnail_types: Vec<ThumbnailType>,
}
