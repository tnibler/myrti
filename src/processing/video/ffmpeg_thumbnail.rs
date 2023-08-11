use eyre::{bail, Context, Result};
use std::{path::Path, process::Stdio};
use tokio::process::Command;
use tracing::instrument;

#[instrument("Take video snapshot")]
pub async fn create_snapshot(video_path: &Path, out_path: &Path) -> Result<()> {
    let exit_status = Command::new("ffmpeg")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-i")
        .arg(video_path)
        .args(&["-ss", "00:00:00.00", "-frames:v", "1"])
        .arg(out_path)
        // .stdout(Stdio::piped())
        // .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to call ffmpeg")?
        .wait()
        .await
        .wrap_err("ffmpeg error")?;
    match exit_status.code() {
        Some(0) => Ok(()),
        Some(_) => {
            bail!("error taking video snapshot: ffmpeg exited with non-zero code")
        }
        None => {
            bail!("error taking video snapshot: ffmpeg exited by signal")
        }
    }
}
