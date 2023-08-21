use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use eyre::{bail, Context, Result};
use tokio::{fs, process::Command};
use tracing::{debug, instrument};

use super::transcode::ffmpeg_command;
pub use crate::catalog::encoding_target::EncodingTarget;

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
pub async fn transcode_and_package(
    input: &Path,
    video_out: &Path,
    audio_out: &Path,
    encoding_target: EncodingTarget,
    mpd_out: &Path,
) -> Result<()> {
    // called when no suitable representation exists at all yet
    // transcode original video into one target codec only
    // and package that representation only

    // Supposedly you could use an mkfifo between ffmpeg and shaka-packager, but it errored out in
    // my superficial testing. See: https://shaka-project.github.io/shaka-packager/html/tutorials/ffmpeg_piping.html
    let ffmpeg_out_dir = tempfile::tempdir().wrap_err("could not create temp directory")?;
    let ffmpeg_out_path = ffmpeg_out_dir.path().join("out.mp4");
    let mut ffmpeg_command = ffmpeg_command(input, &ffmpeg_out_path, &encoding_target);
    let result = ffmpeg_command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("error calling ffmpeg")?
        .wait()
        .await?;
    if !result.success() {
        bail!("ffmpeg exited with an error")
    }
    let reprs = [
        RepresentationInput {
            path: input.to_owned(),
            ty: RepresentationType::Video,
            out_path: video_out.to_owned(),
        },
        RepresentationInput {
            path: input.to_owned(),
            ty: RepresentationType::Audio,
            out_path: audio_out.to_owned(),
        },
    ];
    shaka_package(&reprs, mpd_out).await?;
    Ok(())
}
