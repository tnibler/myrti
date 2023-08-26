use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use eyre::{bail, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument, Instrument};

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
pub async fn shaka_package(reprs: &[RepresentationInput]) -> Result<()> {
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
    // let mpd_out_path = mpd_out_path.to_str().unwrap().to_owned();
    // command.arg(format!("--mpd_output={}", mpd_out_path));
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
    shaka_package_with_mpd(&reprs, mpd_out).await?;
    Ok(())
}
