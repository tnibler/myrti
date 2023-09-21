pub mod hash;
pub mod image;
pub mod indexing;
pub mod media_metadata;
pub mod video;

#[cfg(not(feature = "mock-commands"))]
pub mod commands {
    pub use super::image::image_conversion::ConvertImage;
    pub use super::image::thumbnail::GenerateThumbnail;
    pub use super::video::ffmpeg::FFmpeg;
    pub use super::video::ffmpeg_into_shaka::FFmpegIntoShaka;
    pub use super::video::mpd_generator::MpdGenerator;
    pub use super::video::shaka::ShakaPackager;
    pub use super::video::shaka_into_ffmpeg::ShakaIntoFFmpeg;
}

#[cfg(feature = "mock-commands")]
pub mod commands {
    pub use super::image::image_conversion::ConvertImageMock as ConvertImage;
    pub use super::image::thumbnail::GenerateThumbnailMock as GenerateThumbnail;
    pub use super::video::ffmpeg::FFmpegMock as FFmpeg;
    pub use super::video::ffmpeg_into_shaka::FFmpegIntoShakaMock as FFmpegIntoShaka;
    pub use super::video::mpd_generator::MpdGeneratorMock as MpdGenerator;
    pub use super::video::shaka::ShakaPackagerMock as ShakaPackager;
    pub use super::video::shaka_into_ffmpeg::ShakaIntoFFmpegMock as ShakaIntoFFmpeg;
}

#[cfg(test)]
mod test;
