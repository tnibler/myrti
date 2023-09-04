use std::path::{Path, PathBuf};

use eyre::{bail, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument, Instrument};

pub use crate::catalog::encoding_target::VideoEncodingTarget;

#[instrument()]
pub async fn generate_mpd(media_info_paths: &[PathBuf], mpd_out_path: &Path) -> Result<()> {
    let input_str = media_info_paths
        .iter()
        .map(|p| p.to_str().unwrap().to_owned())
        .collect::<Vec<_>>()
        .join(",");
    let mpd_out_str = mpd_out_path.to_str().unwrap();
    // TODO don't hardcode this path
    let mut command = Command::new("./mpd_generator");
    command
        .arg(format!("--input={}", input_str))
        .arg(format!("--output={}", mpd_out_str));
    debug!(?command, "Invoking mpd_generator");
    let result = command
        .spawn()
        .wrap_err("error calling mpd_generator")?
        .wait()
        .in_current_span()
        .await?;
    // FIXME mpd_generator just skips input media_info files that fail to open/don't exist
    // but carries on and exits with 0 so this check doesn't do anything
    if !result.success() {
        bail!("mpd_generator exited with error");
    }
    Ok(())
}
