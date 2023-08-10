use std::path::{Path, PathBuf};

use eyre::{bail, Context, Result};
use tokio::{fs, process::Command};
use tracing::debug;

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
pub async fn shaka_package(reprs: &[RepresentationInput], mpd_out_path: &Path) -> Result<()> {
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
    debug!(?command, "Invoking shaka-packager");
    let result = command
        // .stdout(Stdio::piped())
        .spawn()
        .wrap_err("error calling shaka packager")?
        .wait()
        .await?;
    if !matches!(result.code(), Some(0)) {
        bail!("shaka packager exited with error");
    }
    Ok(())
}
