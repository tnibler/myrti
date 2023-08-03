use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

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
}

// FIXME this doesn't work for video files with no audio/video streams
// it assumes one video and one audio stream
pub async fn shaka_package(
    reprs: &[RepresentationInput],
    out_dir: &Path,
    mpd_name: &str,
) -> Result<()> {
    fs::create_dir_all(out_dir).await?;
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
        let filename = match repr.ty {
            RepresentationType::Video => {
                repr.path.file_name().unwrap().to_str().unwrap().to_string()
            }
            RepresentationType::Audio => format!(
                "{}_audio.{}",
                repr.path.file_stem().unwrap().to_str().unwrap(),
                repr.path.extension().unwrap().to_str().unwrap()
            ),
        };
        let out_path = out_dir.join(&filename).to_str().unwrap().to_owned();
        command.arg(format!(
            "in={},stream={},output={}",
            path_str, stream, out_path
        ));
    }
    let mpd_out_path = out_dir.join(mpd_name).to_str().unwrap().to_owned();
    command.arg(format!("--mpd_output={}", mpd_out_path));
    debug!(?command, "Invoking shaka-packager");
    let result = command
        // .stdout(Stdio::piped())
        .spawn()
        .wrap_err("error calling shaka packager")?
        .wait()
        .await?;
    if !matches!(result.code(), Some(0)) {
        panic!();
        bail!("shaka packager exited with error");
    }
    Ok(())
}
