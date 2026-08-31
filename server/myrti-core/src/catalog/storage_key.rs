use std::fmt;

use myrti_data::model::{AlbumId, FileId, ThumbnailFormat, ThumbnailType};

use super::image_conversion_target::{ImageConversionTarget, ImageFormatTarget};

pub fn dash_file(file_id: FileId, filename: fmt::Arguments) -> String {
    format!("dash/{}/{}", file_id.0, filename)
}

/// returned key is always in the set of keys returned by `dash_file`
pub fn mpd_manifest(file_id: FileId) -> String {
    dash_file(file_id, format_args!("stream.mpd"))
}

pub fn thumbnail(asset_id: FileId, ty: ThumbnailType, format: ThumbnailFormat) -> String {
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
pub fn image_representation(file_id: FileId, target: &ImageConversionTarget) -> String {
    let ext = image_file_extension(&target.format);
    match target.scale {
        None => format!("image/{}.{}", file_id.0, ext),
        Some(scale) => format!("image/{}_{}x.{}", file_id.0, scale, ext),
    }
}

pub fn album_thumbnail(album_id: AlbumId, format: ThumbnailFormat) -> String {
    let ext = match format {
        ThumbnailFormat::Webp => format_args!("webp"),
        ThumbnailFormat::Avif => format_args!("avif"),
    };
    format!("album_thumb/{}.{}", album_id.0, ext)
}

fn image_file_extension(target: &ImageFormatTarget) -> &'static str {
    match target {
        ImageFormatTarget::JPEG(_) => "jpg",
        ImageFormatTarget::AVIF(_) => "avif",
        ImageFormatTarget::WEBP => "webp",
    }
}
