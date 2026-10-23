use std::{ffi::OsString, process::Stdio};

use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{Context, Result, eyre};
use tokio::process::Command;

use crate::processing::process_control::{ProcessControlReceiver, run_process};

#[derive(thiserror::Error, Debug)]
pub enum FFmpegError {
    #[error("Error starting FFmpeg")]
    ErrorStarting,
    #[error("FFmpeg exited by signal")]
    TerminatedBySignal,
}

#[tracing::instrument(name = "ffmpeg", skip(control_recv), level = "trace")]
pub async fn run_ffmpeg(
    input: &str,
    output: &Path,
    pre_input_flags: &[OsString],
    flags: &[OsString],
    (ffmpeg_path, args): (&Path, &[String]),
    control_recv: &mut ProcessControlReceiver,
) -> Result<()> {
    let mut command = Command::new(ffmpeg_path);
    command
        .args(args)
        .arg("-nostdin")
        .arg("-y")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.args(pre_input_flags.iter());
    command.arg("-i").arg(input);
    command.args(flags.iter());
    command.arg(output);
    tracing::debug!(command = ?command.as_std(), "Invoking ffmpeg");
    let child = command.spawn().wrap_err(FFmpegError::ErrorStarting)?;
    run_process(child, Default::default(), control_recv)
        .await
        .wrap_err("error running ffmpeg")?;
    Ok(())
}

pub async fn ffmpeg_package_dash(
    input: &Path,
    output_dir: &Path,
    mpd_name: &str,
    segment_duration: f32,
    (ffmpeg_path, args): (&Path, &[String]),
    control_recv: &mut ProcessControlReceiver,
) -> Result<PathBuf> {
    let segment_name = format!(
        "{}_dashinit.mp4",
        input.file_stem().ok_or(eyre!("bad input filename"))?
    );
    let mut command = Command::new(ffmpeg_path);
    command.args(args).args(["-nostdin", "-y", "-v", "warning", "-hide_banner"]);
    // .stdout(Stdio::null())
    // .stderr(Stdio::null());
    command
        .arg("-i")
        .arg(input)
        .args([
            "-c:a",
            "copy",
            "-c:v",
            "copy",
            "-f",
            "dash",
            "-dash_segment_type",
            "mp4",
            "-global_sidx",
            "1",
            "-seg_duration",
        ])
        .arg(segment_duration.to_string())
        .args(["-single_file", "1", "-single_file_name"])
        .arg(&segment_name)
        .arg(output_dir.join(mpd_name));
    tracing::debug!(command = ?command.as_std(), "Invoking ffmpeg");
    let child = command.spawn().wrap_err(FFmpegError::ErrorStarting)?;
    run_process(child, Default::default(), control_recv)
        .await
        .wrap_err("error running ffmpeg")?;
    Ok(output_dir.join(segment_name))
}
