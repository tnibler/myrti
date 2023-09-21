use std::path::PathBuf;

use async_trait::async_trait;
use eyre::{Context, Result};
use tracing::Instrument;

use crate::core::storage::Storage;

use super::{
    ffmpeg::{FFmpeg, FFmpegLocalOutputTrait, FFmpegTrait},
    shaka::{RepresentationType, ShakaPackager, ShakaPackagerTrait, ShakaResult},
    streams::FFProbeStreamsTrait,
    transcode::{ffmpeg_audio_flags, ffmpeg_video_flags, ProduceAudio, ProduceVideo},
    FFProbe, FFProbeStreams, VideoStream,
};

#[async_trait]
pub trait FFmpegIntoShakaFFmpegTrait {
    type Next: FFmpegIntoShakaTrait;

    fn new(input: PathBuf, video: Option<&ProduceVideo>, audio: Option<&ProduceAudio>) -> Self;

    async fn run_ffmpeg(self) -> Result<Self::Next>;
}

#[async_trait]
pub trait FFmpegIntoShakaTrait {
    async fn run_shaka_packager(
        &self,
        repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
    ) -> Result<ShakaResult>;

    async fn ffprobe_get_streams(&self) -> Result<FFProbeStreams>;
}

pub struct FFmpegIntoShaka {
    input: PathBuf,
    ffmpeg: FFmpeg,
}

#[async_trait]
impl FFmpegIntoShakaFFmpegTrait for FFmpegIntoShaka {
    type Next = FFmpegIntoShakaAfterFFmpeg;

    fn new(input: PathBuf, video: Option<&ProduceVideo>, audio: Option<&ProduceAudio>) -> Self {
        let pre_input_flags = Vec::default();
        let mut flags = match video {
            Some(video) => ffmpeg_video_flags(video),
            None => Vec::default(),
        };
        if let Some(audio) = audio {
            flags.append(&mut ffmpeg_audio_flags(audio));
        }
        let ffmpeg = FFmpeg::new(
            pre_input_flags,
            flags.into_iter().map(|s| s.into()).collect(),
        );
        Self { input, ffmpeg }
    }

    async fn run_ffmpeg(self) -> Result<Self::Next> {
        let ffmpeg_out_path = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .wrap_err("error creating temp directory")?
            .into_temp_path();
        self.ffmpeg
            .run_with_local_output(&self.input, &ffmpeg_out_path)
            .await?;
        Ok(FFmpegIntoShakaAfterFFmpeg { ffmpeg_out_path })
    }
}

pub struct FFmpegIntoShakaAfterFFmpeg {
    ffmpeg_out_path: tempfile::TempPath,
}

#[async_trait]
impl FFmpegIntoShakaTrait for FFmpegIntoShakaAfterFFmpeg {
    async fn run_shaka_packager(
        &self,
        repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
    ) -> Result<ShakaResult> {
        ShakaPackager::run(&self.ffmpeg_out_path, repr_type, output_key, storage).await
    }

    async fn ffprobe_get_streams(&self) -> Result<FFProbeStreams> {
        FFProbe::streams(&self.ffmpeg_out_path)
            .in_current_span()
            .await
    }
}

#[cfg(feature = "mock-commands")]
pub struct FFmpegIntoShakaMock {
    video: Option<ProduceVideo>,
    audio: Option<ProduceAudio>,
}

#[cfg(feature = "mock-commands")]
#[async_trait]
impl FFmpegIntoShakaFFmpegTrait for FFmpegIntoShakaMock {
    type Next = FFmpegIntoShakaMock;

    fn new(input: PathBuf, video: Option<&ProduceVideo>, audio: Option<&ProduceAudio>) -> Self {
        Self {
            video: video.cloned(),
            audio: audio.cloned(),
        }
    }

    async fn run_ffmpeg(self) -> Result<Self::Next> {
        Ok(self)
    }
}

#[cfg(feature = "mock-commands")]
#[async_trait]
impl FFmpegIntoShakaTrait for FFmpegIntoShakaMock {
    async fn run_shaka_packager(
        &self,
        repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
    ) -> Result<ShakaResult> {
        super::shaka::ShakaPackagerMock::run(
            &PathBuf::from("MOCK_PATH"),
            repr_type,
            output_key,
            storage,
        )
        .await
    }

    async fn ffprobe_get_streams(&self) -> Result<FFProbeStreams> {
        use super::AudioStream;
        Ok(FFProbeStreams {
            video: VideoStream {
                codec_name: "mock_codec".into(),
                width: 1,
                height: 1,
                bitrate: 1,
                rotation: None,
            },
            audio: self.audio.as_ref().map(|_audio| AudioStream {
                codec_name: "mock_codec".into(),
                bitrate: 1,
                channels: 1,
                sample_rate: 1,
            }),
        })
    }
}
