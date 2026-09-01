use std::{ffi::OsString, process::Stdio};

use camino::Utf8Path as Path;
use eyre::{Context, Result};
use tokio::process::Command;

use crate::processing::process_control::{ProcessControlReceiver, run_process};

#[derive(thiserror::Error, Debug)]
pub enum FFmpegError {
    #[error("Error starting FFmpeg")]
    ErrorStarting,
    #[error("FFmpeg exited by signal")]
    TerminatedBySignal,
}

#[tracing::instrument(name = "ffmpeg", skip(control_recv), level = "trace")]
pub async fn run_ffmpeg(
    input: &str,
    output: &Path,
    pre_input_flags: &[OsString],
    flags: &[OsString],
    ffmpeg_bin_path: Option<&Path>,
    control_recv: &mut ProcessControlReceiver,
) -> Result<()> {
    let mut command = Command::new(ffmpeg_bin_path.unwrap_or("ffmpeg".into()));
    command
        .arg("-nostdin")
        .arg("-y")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.args(pre_input_flags.iter());
    command.arg("-i").arg(input);
    command.args(flags.iter());
    command.arg(output);
    tracing::debug!(command = ?command.as_std(), "Invoking ffmpeg");
    let child = command.spawn().wrap_err(FFmpegError::ErrorStarting)?;
    run_process(child, Default::default(), control_recv)
        .await
        .wrap_err("error running ffmpeg")?;
    Ok(())
}
