pub mod hash;
pub mod image;
pub mod indexing;
pub mod media_metadata;
pub mod process_control;
pub mod startup_self_check;
pub mod video;

#[cfg(not(feature = "mock-commands"))]
pub mod commands {
    pub use super::image::image_conversion::ConvertImage;
    pub use super::image::thumbnail::GenerateThumbnail;
    pub use super::video::ffmpeg::FFmpeg;
}

#[cfg(feature = "mock-commands")]
pub mod commands {
    pub use super::image::image_conversion::ConvertImageMock as ConvertImage;
    pub use super::image::thumbnail::GenerateThumbnailMock as GenerateThumbnail;
    pub use super::video::ffmpeg::FFmpegMock as FFmpeg;
}

#[cfg(test)]
mod test;
