use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{eyre, Context, Result};
use itertools::Itertools;
use tokio::process::Command;

use crate::processing::process_control::{run_process, ProcessControlReceiver, ProcessResult};

#[derive(Debug, Clone)]
pub struct CreateGHIOptions {
    pub segment_duration: i32,
    pub video_rep_id: Option<String>,
    pub audio_rep_id: Option<String>,
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
    let input_arg = match (opts.video_rep_id.as_deref(), opts.audio_rep_id.as_deref()) {
        (None, None) => return Err(eyre!("at least one video or audio track must be selected")),
        (Some(v), None) => format!("{}:#Representation=(video){}:tkid=video", input, v),
        // limiting to tkid=audio makes segment creation with sn=XX step fail, keeping video fixes it.
        // the ignore representation will be dropped when manifests are merged
        (None, Some(a)) => format!("{}:#Representation=(video)ignored,(audio){}", input, a),
        (Some(v), Some(a)) => format!("{}:#Representation=(video){},(audio){}", input, v, a),
    };
    command.args([
        "-i",
        &input_arg,
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
        command.arg(format!("{}:base={}:stl=true", opts.mpd_out_path, base));
    } else {
        command.arg(format!("{}:stl=true:profile=live", opts.mpd_out_path));
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

#[tracing::instrument(skip(control_recv), err)]
pub async fn create_segment(
    ghi_path: &Path,
    out_dir: &Path,
    rep_id: &str,
    segment: i32,
    gpac_bin_path: Option<&Path>,
    control_recv: &mut ProcessControlReceiver,
) -> Result<()> {
    let mut command = Command::new(gpac_bin_path.unwrap_or("gpac".into()));
    command.args([
        "-i",
        if segment == 0 {
            format!("{}:rep={}:gm=init", ghi_path, rep_id,)
        } else {
            format!("{}:rep={}:sn={segment}", ghi_path, rep_id,)
        }
        .as_str(),
        "-o",
        out_dir.join("unused.mpd").as_str(),
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
    }
}

#[derive(Debug, Clone)]
pub struct GpacDashResult {
    pub mpd_path: PathBuf,
    pub mp4_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DasherOptions<'a> {
    pub mpd_name: &'a str,
    pub base_url: Option<&'a str>,
}

#[tracing::instrument(skip(control_recv))]
pub async fn run_dasher(
    input_path: &Path,
    out_dir: &Path,
    opts: DasherOptions<'_>,
    gpac_bin_path: Option<&Path>,
    control_recv: &mut ProcessControlReceiver,
) -> Result<GpacDashResult> {
    let mut command = Command::new(gpac_bin_path.unwrap_or("gpac".into()));
    command.current_dir(out_dir).args([
        "-i",
        input_path.as_str(),
        "-o",
        format!(
            "{}:profile=onDemand{}",
            opts.mpd_name,
            opts.base_url
                .map(|url| format!(":base={}", url))
                .unwrap_or(String::new())
        )
        .as_str(),
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
        mpd_path: out_dir.join(opts.mpd_name),
        mp4_path: out_dir.join(mp4_name),
    })
}
