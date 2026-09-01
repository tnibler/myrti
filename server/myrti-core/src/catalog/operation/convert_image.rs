use myrti_data::model::FileId;

use crate::catalog::image_conversion_target::ImageConversionTarget;

#[derive(Debug, Clone)]
pub struct ConvertImage {
    pub file_id: FileId,
    pub target: ImageConversionTarget,
    pub output_file_key: String,
}
