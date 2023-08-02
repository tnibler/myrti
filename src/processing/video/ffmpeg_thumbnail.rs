use crate::processing::image::{self, OutDimension};
use eyre::{bail, eyre, Context, Result};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};
use tokio::process::Command;

pub async fn create_snapshot(video_path: &Path, out_path: &Path) -> Result<()> {
    let exit_status = Command::new("ffmpeg")
        .arg("-i")
        .arg(video_path)
        .args(&["-ss", "00:00:00.00", "-vframes", "1"])
        .arg(out_path)
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
