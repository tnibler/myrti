pub mod ffmpeg;
pub mod ffmpeg_into_shaka;
mod ffprobe;
pub mod mpd_generator;
pub mod shaka;
pub mod shaka_into_ffmpeg;
pub mod transcode;
pub use ffprobe::*;

#[cfg(not(feature = "mock-commands"))]
pub mod commands {
    pub use super::ffmpeg::FFmpeg;
    pub use super::ffmpeg_into_shaka::FFmpegIntoShaka;
    pub use super::mpd_generator::MpdGenerator;
    pub use super::shaka::ShakaPackager;
    pub use super::shaka_into_ffmpeg::ShakaIntoFFmpeg;
}

#[cfg(feature = "mock-commands")]
pub mod commands {
    pub use super::ffmpeg::FFmpegMock as FFmpeg;
    pub use super::ffmpeg_into_shaka::FFmpegIntoShakaMock as FFmpegIntoShaka;
    pub use super::mpd_generator::MpdGeneratorMock as MpdGenerator;
    pub use super::mpd_generator::MpdGeneratorMock as MpdGenerator;
    pub use super::shaka::ShakaPackagerMock as ShakaPackager;
    pub use super::shaka_into_ffmpeg::ShakaIntoFFmpegMock as ShakaIntoFFmpeg;
}
