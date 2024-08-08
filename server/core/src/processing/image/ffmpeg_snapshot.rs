use std::process::Stdio;

use camino::Utf8Path as Path;
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::instrument;

use crate::processing::{
    process_control::{run_process, ProcessControlReceiver, ProcessResult},
    video::ffmpeg::FFmpegError,
};

#[instrument]
pub async fn ffmpeg_snapshot(
    video_path: &Path,
    output: &Path,
    ffmpeg_bin_path: Option<&str>,
    control_recv: &mut ProcessControlReceiver,
) -> Result<()> {
    let child = Command::new(ffmpeg_bin_path.unwrap_or("ffmpeg"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(["-nostdin", "-y", "-hide_banner"])
        .arg("-i")
        .arg(video_path)
        .args(["-ss", "00:00:00.00", "-frames:v", "1"])
        .arg(output)
        .spawn()
        .wrap_err(FFmpegError::ErrorStarting)?;

    match run_process(child, control_recv).await {
        ProcessResult::RanToEnd(output) if output.status.success() => Ok(()),
        ProcessResult::RanToEnd(_output) => Err(eyre!("ffmpeg exited with an error")),
        ProcessResult::TerminatedBySignal(_) => Err(FFmpegError::TerminatedBySignal.into()),
        ProcessResult::OtherError(err) => Err(err.wrap_err("error running ffmpeg")),
    }
}
