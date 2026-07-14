use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{eyre, Context, Result};
use itertools::Itertools;
use tokio::process::Command;

use crate::processing::process_control::{run_process, ProcessControlReceiver, ProcessResult};

#[derive(Debug, Clone)]
pub struct CreateGHIOptions {
    pub segment_duration: i32,
    pub video_rep_id: String,
    pub audio_rep_id: String,
    pub mpd_base_url: Option<String>,
    pub ghi_out_path: PathBuf,
    pub mpd_out_path: PathBuf,
}

#[tracing::instrument(skip(control_recv))]
pub async fn create_ghi_and_manifest(
    input: &Path,
    opts: &CreateGHIOptions,
    gpac_bin_path: Option<&Path>,
    control_recv: &mut ProcessControlReceiver,
) -> Result<()> {
    let mut command = Command::new(gpac_bin_path.unwrap_or("gpac".into()));
    command.args([
        "-i",
        &format!(
            "{}:#Representation=(video){},(audio){}",
            input, opts.video_rep_id, opts.audio_rep_id
        ),
        "-o",
        &format!("{}:segdur={}", opts.ghi_out_path, opts.segment_duration),
    ]);
    tracing::debug!(?command);
    let child = command.spawn().context("error calling gpac")?;
    match run_process(child, control_recv).await {
        ProcessResult::RanToEnd(output) if output.status.success() => Ok(()),
        ProcessResult::RanToEnd(output) => Err(eyre!(
            "gpac exited with an error:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )),
        ProcessResult::TerminatedBySignal(_) => Err(eyre!("TerminatedBySignal")),
        ProcessResult::OtherError(err) => Err(err.wrap_err("error running gpac")),
    }?;

    let mut command = Command::new(gpac_bin_path.unwrap_or("gpac".into()));
    command.args(["-i", &format!("{}:gm=main", opts.ghi_out_path,), "-o"]);
    if let Some(base) = opts.mpd_base_url.as_ref() {
        command.arg(format!("{}:base={}", opts.mpd_out_path, base));
    } else {
        command.arg(&opts.mpd_out_path);
    }
    tracing::debug!(?command);
    let child = command.spawn().context("error calling gpac")?;
    match run_process(child, control_recv).await {
        ProcessResult::RanToEnd(output) if output.status.success() => Ok(()),
        ProcessResult::RanToEnd(output) => Err(eyre!(
            "gpac exited with an error:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )),
        ProcessResult::TerminatedBySignal(_) => Err(eyre!("TerminatedBySignal")),
        ProcessResult::OtherError(err) => Err(err.wrap_err("error running gpac")),
    }
}

pub async fn create_segment(
    ghi_path: &Path,
    out_dir: &Path,
    rep_id: &str,
    segments: &[i32],
    gpac_bin_path: Option<&Path>,
    control_recv: &mut ProcessControlReceiver,
) -> Result<()> {
    let mut command = Command::new(gpac_bin_path.unwrap_or("gpac".into()));
    command.args([
        "-i",
        &format!(
            "{}:rep={}{}",
            ghi_path,
            rep_id,
            segments.iter().map(|i| i.to_string()).join(":sn=")
        ),
        "-o",
        out_dir.join("unused.mpd").as_str(),
    ]);
    let child = command.spawn().context("error calling gpac")?;
    match run_process(child, control_recv).await {
        ProcessResult::RanToEnd(output) if output.status.success() => Ok(()),
        ProcessResult::RanToEnd(output) => Err(eyre!(
            "gpac exited with an error:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )),
        ProcessResult::TerminatedBySignal(_) => Err(eyre!("TerminatedBySignal")),
        ProcessResult::OtherError(err) => Err(err.wrap_err("error running gpac")),
    }
}

#[derive(Debug, Clone)]
pub struct GpacDashResult {
    pub mpd_path: PathBuf,
    pub mp4_path: PathBuf,
}

#[tracing::instrument(skip(control_recv))]
pub async fn run_dasher(
    input_path: &Path,
    out_dir: &Path,
    mpd_name: &str,
    gpac_bin_path: Option<&Path>,
    control_recv: &mut ProcessControlReceiver,
) -> Result<GpacDashResult> {
    let mut command = Command::new(gpac_bin_path.unwrap_or("gpac".into()));
    command.current_dir(out_dir).args([
        "-i",
        input_path.as_str(),
        "-o",
        format!("{}:profile=onDemand", mpd_name).as_str(),
    ]);
    tracing::debug!(?command);
    let child = command.spawn().context("error calling gpac")?;
    match run_process(child, control_recv).await {
        ProcessResult::RanToEnd(output) if output.status.success() => Ok(()),
        ProcessResult::RanToEnd(output) => Err(eyre!(
            "gpac exited with an error:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )),
        ProcessResult::TerminatedBySignal(_) => Err(eyre!("TerminatedBySignal")),
        ProcessResult::OtherError(err) => Err(err.wrap_err("error running gpac")),
    }?;
    let mp4_name = format!(
        "{}_dashinit.mp4",
        input_path.file_stem().ok_or(eyre!("bad mpd filename"))?
    );
    Ok(GpacDashResult {
        mpd_path: out_dir.join(mpd_name),
        mp4_path: out_dir.join(mp4_name),
    })
}
