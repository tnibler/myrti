use std::path::{Path, PathBuf};

use eyre::{bail, Context, Result};
use tokio::{fs, process::Command};

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

pub async fn shaka_package(reprs: &[RepresentationInput], out_dir: &Path) -> Result<()> {
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
            RepresentationType::Audio => repr
                .path
                .parent()
                .unwrap()
                .join(format!(
                    "{}_audio.{}",
                    repr.path.file_stem().unwrap().to_str().unwrap(),
                    repr.path.extension().unwrap().to_str().unwrap()
                ))
                .to_str()
                .unwrap()
                .to_string(),
        };
        let out_path = out_dir.join(&filename).to_str().unwrap().to_owned();
        command.arg(format!(
            "in={},stream={},output={}",
            path_str, stream, out_path
        ));
        let mpd_out_path = out_dir.join("stream.mpd").to_str().unwrap().to_owned();
        command.arg(format!("--mpd_output={}", mpd_out_path));
    }
    let result = command
        .spawn()
        .wrap_err("error calling shaka packager")?
        .wait()
        .await?;
    if !matches!(result.code(), Some(0)) {
        bail!("shaka packager exited with error");
    }
    Ok(())
}
