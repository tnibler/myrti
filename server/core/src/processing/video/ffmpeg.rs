use std::{ffi::OsString, process::Stdio};

use async_trait::async_trait;
use camino::Utf8Path as Path;
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument};

use crate::{
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    processing::process_control::{run_process, ProcessControlReceiver, ProcessResult},
};

#[derive(thiserror::Error, Debug)]
pub enum FFmpegError {
    #[error("Error starting FFmpeg")]
    ErrorStarting,
    #[error("FFmpeg exited by signal")]
    TerminatedBySignal,
}

pub trait CommandInputOutput {}

#[async_trait]
pub trait FFmpegLocalOutputTrait {
    async fn run_with_local_output(
        &self,
        input: &str,
        output: &Path,
        ffmpeg_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
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
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<()>;
}

pub struct FFmpeg {
    pre_input_flags: Vec<OsString>,
    flags: Vec<OsString>,
}

#[async_trait]
impl FFmpegLocalOutputTrait for FFmpeg {
    #[instrument(err, name = "ffmpeg", skip(self, control_recv))]
    async fn run_with_local_output(
        &self,
        input: &str,
        output: &Path,
        ffmpeg_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
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
        let child = command.spawn().wrap_err(FFmpegError::ErrorStarting)?;
        match run_process(child, control_recv).await {
            ProcessResult::RanToEnd(output) if output.status.success() => Ok(()),
            ProcessResult::RanToEnd(_output) => Err(eyre!("ffmpeg exited with an error")),
            ProcessResult::TerminatedBySignal(_) => Err(FFmpegError::TerminatedBySignal.into()),
            ProcessResult::OtherError(err) => Err(err.wrap_err("error running ffmpeg")),
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
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        self.run_with_local_output(
            input,
            command_out_file.path(),
            ffmpeg_bin_path,
            control_recv,
        )
        .await?;
        command_out_file.flush_to_storage().await?;
        Ok(())
    }
}

#[allow(inactive_code)]
#[cfg(feature = "mock-commands")]
pub struct FFmpegMock {}

#[allow(inactive_code)]
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
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        command_out_file.flush_to_storage().await?;
        Ok(())
    }
}
