use std::{ffi::OsStr, path::Path, process::Stdio};

use async_trait::async_trait;
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::{instrument, Instrument};

use crate::core::storage::Storage;

use super::{
    shaka::{RepresentationType, ShakaPackager, ShakaPackagerTrait, ShakaResult},
    streams::FFProbeStreamsTrait,
    FFProbe, FFProbeStreams,
};

pub trait FFmpegIntoShakaNew {
    type FFmpegInputFlagTrait: FFmpegIntoShakaInputFlagTrait;
    fn new() -> Self::FFmpegInputFlagTrait;
}

pub trait FFmpegIntoShakaInputFlagTrait {
    type FFmpegFlagTrait: FFmpegIntoShakaFlagTrait;

    fn pre_input_flag<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self;
    fn pre_input_flags<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    fn input(self, path: impl AsRef<Path>) -> Self::FFmpegFlagTrait;
}

#[async_trait]
pub trait FFmpegIntoShakaFlagTrait {
    type ShakaTrait: FFmpegIntoShakaTrait;
    fn flag<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self;
    fn flags<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    async fn run_ffmpeg(self) -> Result<Self::ShakaTrait>;
}

#[async_trait]
pub trait FFmpegIntoShakaTrait {
    async fn run_shaka_packager(
        &self,
        repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
    ) -> Result<ShakaResult>;

    /// calls ffprobe on temporary ffmpeg output file
    async fn ffprobe_get_streams(&self) -> Result<FFProbeStreams>;
}

pub trait FFmpegIntoShakaStep {}

pub struct MissingFFmpegInput {
    ffmpeg_command: Command,
}
impl FFmpegIntoShakaStep for MissingFFmpegInput {}
pub struct WithFFmpegInputArg {
    ffmpeg_command: Command,
}
impl FFmpegIntoShakaStep for WithFFmpegInputArg {}
pub struct FFmpegHasRun {
    ffmpeg_out_path: tempfile::TempPath,
}
impl FFmpegIntoShakaStep for FFmpegHasRun {}

pub struct FFmpegIntoShaka<I: FFmpegIntoShakaStep> {
    step: I,
}

impl FFmpegIntoShakaNew for FFmpegIntoShaka<MissingFFmpegInput> {
    type FFmpegInputFlagTrait = FFmpegIntoShaka<MissingFFmpegInput>;

    fn new() -> Self::FFmpegInputFlagTrait {
        let mut command = Command::new("ffmpeg");
        command
            .arg("-nostdin")
            .arg("-y")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        FFmpegIntoShaka {
            step: MissingFFmpegInput {
                ffmpeg_command: command,
            },
        }
    }
}

impl FFmpegIntoShakaInputFlagTrait for FFmpegIntoShaka<MissingFFmpegInput> {
    type FFmpegFlagTrait = FFmpegIntoShaka<WithFFmpegInputArg>;

    fn pre_input_flag<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.step.ffmpeg_command.arg(arg);
        self
    }

    fn pre_input_flags<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.step.ffmpeg_command.args(args);
        self
    }

    fn input(mut self, path: impl AsRef<Path>) -> Self::FFmpegFlagTrait {
        self.step.ffmpeg_command.arg("-i").arg(path.as_ref());
        FFmpegIntoShaka {
            step: WithFFmpegInputArg {
                ffmpeg_command: self.step.ffmpeg_command,
            },
        }
    }
}

#[async_trait]
impl FFmpegIntoShakaFlagTrait for FFmpegIntoShaka<WithFFmpegInputArg> {
    type ShakaTrait = FFmpegIntoShaka<FFmpegHasRun>;

    fn flag<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.step.ffmpeg_command.arg(arg);
        self
    }

    fn flags<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.step.ffmpeg_command.args(args);
        self
    }

    #[instrument(skip(self))]
    async fn run_ffmpeg(self) -> Result<Self::ShakaTrait> {
        let ffmpeg_out_path = tempfile::Builder::new()
            .suffix(".mp4")
            .tempfile()
            .wrap_err("error creating temp directory")?
            .into_temp_path();
        let mut ffmpeg_command = self.step.ffmpeg_command;
        ffmpeg_command.arg(&ffmpeg_out_path);
        let result = ffmpeg_command
            .spawn()
            .wrap_err("failed to call ffmpeg")?
            .wait()
            .in_current_span()
            .await
            .wrap_err("error waiting for ffmpeg")?;
        if result.success() {
            Ok(FFmpegIntoShaka {
                step: FFmpegHasRun { ffmpeg_out_path },
            })
        } else {
            Err(eyre!("ffmpeg exited with an error"))
        }
    }
}

#[async_trait]
impl FFmpegIntoShakaTrait for FFmpegIntoShaka<FFmpegHasRun> {
    #[instrument(skip(self, storage))]
    async fn run_shaka_packager(
        &self,
        repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
    ) -> Result<ShakaResult> {
        ShakaPackager::run(&self.step.ffmpeg_out_path, repr_type, output_key, storage)
            .in_current_span()
            .await
    }

    async fn ffprobe_get_streams(&self) -> Result<FFProbeStreams> {
        FFProbe::streams(&self.step.ffmpeg_out_path)
            .in_current_span()
            .await
    }
}
