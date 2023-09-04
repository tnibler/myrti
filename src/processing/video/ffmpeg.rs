use std::{
    ffi::{OsStr, OsString},
    path::Path,
    process::Stdio,
};

use async_trait::async_trait;
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument, Instrument};

use crate::core::storage::{Storage, StorageCommandOutput, StorageProvider};

pub trait CommandInputOutput {}

pub trait FFmpegBuilderTrait {
    type FFmpeg: FFmpegTrait;
    fn new() -> Self;
    fn pre_input_flag<S>(self, flag: S) -> Self
    where
        S: AsRef<OsStr>;
    fn pre_input_flags<I, S>(self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    fn flag<S>(self, flag: S) -> Self
    where
        S: AsRef<OsStr>;
    fn flags<I, S>(self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    fn build(self) -> Self::FFmpeg;
}

#[async_trait]
pub trait FFmpegLocalOutputTrait {
    async fn run_with_local_output(&self, input: &Path, output: &Path) -> Result<()>;
}

#[async_trait]
pub trait FFmpegTrait {
    async fn run(&self, input: &Path, output_key: &str, storage: &Storage) -> Result<()>;
}

pub struct FFmpeg {
    pre_input_flags: Vec<OsString>,
    flags: Vec<OsString>,
}

pub struct FFmpegBuilder {
    pre_input_flags: Vec<OsString>,
    flags: Vec<OsString>,
}

impl FFmpegBuilderTrait for FFmpegBuilder {
    type FFmpeg = FFmpeg;

    fn new() -> Self {
        FFmpegBuilder {
            pre_input_flags: Default::default(),
            flags: Default::default(),
        }
    }

    fn pre_input_flag<S>(mut self, flag: S) -> Self
    where
        S: AsRef<OsStr>,
    {
        self.pre_input_flags.push(flag.as_ref().to_owned());
        self
    }

    fn pre_input_flags<I, S>(mut self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.pre_input_flags
            .extend(flags.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    fn flag<S>(mut self, flag: S) -> Self
    where
        S: AsRef<OsStr>,
    {
        self.flags.push(flag.as_ref().to_owned());
        self
    }

    fn flags<I, S>(mut self, flags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.flags
            .extend(flags.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    fn build(self) -> Self::FFmpeg {
        FFmpeg {
            pre_input_flags: self.pre_input_flags,
            flags: self.flags,
        }
    }
}

#[async_trait]
impl FFmpegLocalOutputTrait for FFmpeg {
    #[instrument(name = "ffmpeg", skip(self))]
    async fn run_with_local_output(&self, input: &Path, output: &Path) -> Result<()> {
        let mut command = Command::new("ffmpeg");
        command
            .arg("-nostdin")
            .arg("-y")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.args(self.pre_input_flags.iter());
        command.arg("-i").arg(input);
        command.args(self.flags.iter());
        command.arg(output);
        debug!(command = ?command.as_std(), "Invoking ffmpeg");
        let result = command
            .spawn()
            .wrap_err("error calling ffmpeg")?
            .wait()
            .in_current_span()
            .await
            .wrap_err("error waiting for ffmpeg")?;
        if result.success() {
            Ok(())
        } else {
            Err(eyre!("ffmpeg exited with an error"))
        }
    }
}

#[async_trait]
impl FFmpegTrait for FFmpeg {
    async fn run(&self, input: &Path, output_key: &str, storage: &Storage) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        self.run_with_local_output(input, command_out_file.path())
            .in_current_span()
            .await
    }
}
