use std::{ffi::OsString, process::Stdio};

use async_trait::async_trait;
use camino::Utf8Path as Path;
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument, Instrument};

use crate::core::storage::{Storage, StorageCommandOutput, StorageProvider};

pub trait CommandInputOutput {}

#[async_trait]
pub trait FFmpegLocalOutputTrait {
    async fn run_with_local_output(
        &self,
        input: &str,
        output: &Path,
        ffmpeg_bin_path: Option<&Path>,
    ) -> Result<()>;
}

#[async_trait]
pub trait FFmpegTrait {
    fn new(pre_input_flags: Vec<OsString>, flags: Vec<OsString>) -> Self;
    async fn run(
        &self,
        input: &str,
        output_key: &str,
        storage: &Storage,
        ffmpeg_bin_path: Option<&Path>,
    ) -> Result<()>;
}

pub struct FFmpeg {
    pre_input_flags: Vec<OsString>,
    flags: Vec<OsString>,
}

#[async_trait]
impl FFmpegLocalOutputTrait for FFmpeg {
    #[instrument(name = "ffmpeg", skip(self), level = "debug")]
    async fn run_with_local_output(
        &self,
        input: &str,
        output: &Path,
        ffmpeg_bin_path: Option<&Path>,
    ) -> Result<()> {
        let mut command = Command::new(ffmpeg_bin_path.unwrap_or("ffmpeg".into()));
        command
            .arg("-nostdin")
            .arg("-y")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
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
    fn new(pre_input_flags: Vec<OsString>, flags: Vec<OsString>) -> Self {
        Self {
            pre_input_flags,
            flags,
        }
    }

    async fn run(
        &self,
        input: &str,
        output_key: &str,
        storage: &Storage,
        ffmpeg_bin_path: Option<&Path>,
    ) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        self.run_with_local_output(input, command_out_file.path(), ffmpeg_bin_path)
            .in_current_span()
            .await?;
        command_out_file.flush_to_storage().await?;
        Ok(())
    }
}

#[cfg(feature = "mock-commands")]
pub struct FFmpegMock {}

#[cfg(feature = "mock-commands")]
#[async_trait]
impl FFmpegTrait for FFmpegMock {
    fn new(_pre_input_flags: Vec<OsString>, _flags: Vec<OsString>) -> Self {
        Self {}
    }

    async fn run(
        &self,
        input: &str,
        output_key: &str,
        storage: &Storage,
        ffmpeg_bin_path: Option<&Path>,
    ) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        command_out_file.flush_to_storage().await?;
        Ok(())
    }
}
