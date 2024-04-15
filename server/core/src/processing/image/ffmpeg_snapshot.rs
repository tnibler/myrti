use std::process::Stdio;

use camino::Utf8Path as Path;
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::instrument;

#[instrument]
pub async fn ffmpeg_snapshot(
    video_path: &Path,
    output: &Path,
    ffmpeg_bin_path: Option<&str>,
) -> Result<()> {
    let exit_status = Command::new(ffmpeg_bin_path.unwrap_or("ffmpeg"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("-nostdin")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .args(["-ss", "00:00:00.00", "-frames:v", "1"])
        .arg(output)
        .spawn()
        .wrap_err("failed to call ffmpeg")?
        .wait()
        .await
        .wrap_err("ffmpeg error")?;
    match exit_status.code() {
        Some(0) => Ok(()),
        Some(_) => Err(eyre!(
            "error taking video snapshot: ffmpeg exited with non-zero code"
        )),
        None => Err(eyre!(
            "error taking video snapshot: ffmpeg exited by signal"
        )),
    }
}
