use std::process::Stdio;

use camino::Utf8Path as Path;
use eyre::{Context, Result};
use tokio::process::Command;
use tracing::instrument;

use crate::processing::{
    process_control::{run_process, ProcessControlReceiver, RunProcessOpts},
    video::ffmpeg::FFmpegError,
};

#[instrument(skip(control_recv))]
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

    run_process(child, RunProcessOpts::with_timeout_secs(3600), control_recv)
        .await
        .wrap_err("error taking snapshot with ffmpeg")?;
    Ok(())
}
