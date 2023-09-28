use std::fmt;

use crate::model::{AssetId, ThumbnailFormat, ThumbnailType};

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

pub fn image_representation(asset_id: AssetId, target: &ImageConversionTarget) -> String {
    let ext = image_file_extension(&target.format);
    match target.scale {
        None => format!("image/{}.{}", asset_id.0, ext),
        Some(scale) => format!("image/{}_{}x.{}", asset_id.0, scale, ext),
    }
}

fn image_file_extension(target: &ImageFormatTarget) -> &'static str {
    match target {
        ImageFormatTarget::JPEG(_) => "jpg",
        ImageFormatTarget::AVIF(_) => "avif",
    }
}
