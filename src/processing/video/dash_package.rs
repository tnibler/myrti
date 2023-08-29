use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use eyre::{bail, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument, Instrument};

pub use crate::catalog::encoding_target::VideoEncodingTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepresentationType {
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationInput {
    pub path: PathBuf,
    pub ty: RepresentationType,
    pub out_path: PathBuf,
}

// FIXME this doesn't work for video files with no audio/video streams
// it assumes one video and one audio stream
#[instrument()]
pub async fn shaka_package(
    reprs: &[RepresentationInput],
    working_dir: Option<&Path>,
) -> Result<()> {
    let mut command = Command::new("packager");
    for repr in reprs {
        if !repr.path.is_file() {
            bail!("input paths for segmenting must be files");
        }
        let path_str = repr.path.to_str().unwrap();
        let stream = match repr.ty {
            RepresentationType::Video => "video",
            RepresentationType::Audio => "audio",
        };
        let out_path = repr.out_path.to_str().unwrap().to_owned();
        command.arg(format!(
            "in={},stream={},output={}",
            path_str, stream, out_path
        ));
    }
    command.arg("--output_media_info");
    if let Some(wd) = working_dir {
        command.current_dir(wd);
    }

    debug!(?command, "Invoking shaka-packager");
    let result = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("error calling shaka packager")?
        .wait()
        .await?;
    if !matches!(result.code(), Some(0)) {
        bail!("shaka packager exited with error");
    }
    Ok(())
}

#[instrument()]
pub async fn shaka_package_with_mpd(
    reprs: &[RepresentationInput],
    mpd_out_path: &Path,
) -> Result<()> {
    let mut command = Command::new("packager");
    for repr in reprs {
        if !repr.path.is_file() {
            bail!("input paths for segmenting must be files");
        }
        let path_str = repr.path.to_str().unwrap();
        let stream = match repr.ty {
            RepresentationType::Video => "video",
            RepresentationType::Audio => "audio",
        };
        let out_path = repr.out_path.to_str().unwrap().to_owned();
        command.arg(format!(
            "in={},stream={},output={}",
            path_str, stream, out_path
        ));
    }
    let mpd_out_path = mpd_out_path.to_str().unwrap().to_owned();
    command.arg(format!("--mpd_output={}", mpd_out_path));
    command.arg("--output_media_info");
    debug!(?command, "Invoking shaka-packager");
    let result = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("error calling shaka packager")?
        .wait()
        .await?;
    if !result.success() {
        bail!("shaka packager exited with error");
    }
    Ok(())
}

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
