use std::fmt;

use crate::model::{
    AssetId, ImageRepresentation, ImageRepresentationId, ThumbnailFormat, ThumbnailType,
};

use super::image_conversion_target::{ImageConversionTarget, ImageFormatTarget};

pub fn dash_file(asset_id: AssetId, filename: fmt::Arguments) -> String {
    format!("dash/{}/{}", asset_id.0, filename)
}

/// returned key is always in the set of keys returned by `dash_file`
pub fn mpd_manifest(asset_id: AssetId) -> String {
    dash_file(asset_id, format_args!("stream.mpd"))
}

pub fn thumbnail(asset_id: AssetId, ty: ThumbnailType, format: ThumbnailFormat) -> String {
    let size = match ty {
        ThumbnailType::SmallSquare => format_args!("_sm"),
        ThumbnailType::LargeOrigAspect => format_args!(""),
    };
    let extension = match format {
        ThumbnailFormat::Webp => format_args!("webp"),
        ThumbnailFormat::Avif => format_args!("avif"),
    };
    format!("thumb/{}{}.{}", asset_id.0, size, extension)
}

// format_name is not really needed, and forces us to do a db query for every
// image represenation API request
// It can be removed at some point, but for now I like having the file extension
pub fn image_representation(
    asset_id: AssetId,
    repr_id: ImageRepresentationId,
    format_name: &str,
) -> String {
    format!("image/{}_{}.{}", asset_id.0, repr_id.0, format_name)
}

pub fn image_file_extension(target: &ImageFormatTarget) -> &'static str {
    match target {
        ImageFormatTarget::JPEG(_) => "jpg",
        ImageFormatTarget::AVIF(_) => "avif",
    }
}
