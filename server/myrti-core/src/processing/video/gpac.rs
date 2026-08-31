use std::os::unix::fs::MetadataExt;

use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{Context, Result, eyre};
use tokio::process::Command;

use crate::processing::process_control::{ProcessControlReceiver, RunProcessOpts, run_process};

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
        &format!(
            "{}:stl=true:segdur={}",
            opts.ghi_out_path, opts.segment_duration
        ),
    ]);
    tracing::debug!(?command);
    let child = command.spawn().context("error calling gpac")?;
    run_process(child, RunProcessOpts::with_timeout_secs(3600), control_recv)
        .await
        .wrap_err("Error running gpac to create ghi index")?;

    let mut command = Command::new(gpac_bin_path.unwrap_or("gpac".into()));
    // gm=main produces broken init segments even though docs say it only writes manifests
    command.args(["-i", &format!("{}:gm=all", opts.ghi_out_path,), "-o"]);
    if let Some(base) = opts.mpd_base_url.as_ref() {
        command.arg(format!("{}:base={}:stl=true", opts.mpd_out_path, base));
    } else {
        command.arg(format!("{}:stl=true:profile=live", opts.mpd_out_path));
    }
    tracing::debug!(?command);
    let child = command.spawn().context("error calling gpac")?;
    if let Err(err) = run_process(child, RunProcessOpts::with_timeout_secs(300), control_recv)
        .await
        .wrap_err("error writing MPD manifest and mp4 init segment with gpac")
    {
        // I don't even know *_* Sometimes gpac outputs this:
        // [MP4Mux] PID configuration not known after EOS, aborting initial timing sync
        // [MP4Mux] Unable to setup fragmentation for track ID 0: Bad Parameter
        // which seems to only happen for video streams (some Pixel 4A HEVC files).
        // Since the video representation is included even when we only need audio (see above),
        // it will error out but ostensibly it has already written everything we need.
        let base_dir = opts.mpd_out_path.parent().unwrap();
        let video_ok = if let Some(rep_id) = &opts.video_rep_id {
            tokio::fs::metadata(base_dir.join(format!("{}-init.mp4", rep_id)))
                .await
                .is_ok_and(|md| md.size() > 100)
        } else {
            true
        };
        let audio_ok = if let Some(rep_id) = &opts.audio_rep_id {
            tokio::fs::metadata(base_dir.join(format!("{}-init.mp4", rep_id)))
                .await
                .is_ok_and(|md| md.size() > 100)
        } else {
            true
        };
        let worked_despite_error = tokio::fs::try_exists(&opts.mpd_out_path)
            .await
            .is_ok_and(|b| b)
            && video_ok
            && audio_ok;
        if !worked_despite_error {
            return Err(err);
        }
    }
    Ok(())
}

#[tracing::instrument(skip(control_recv), err, level = "trace")]
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
    tracing::trace!(?command);
    let child = command.spawn().context("error calling gpac")?;
    run_process(child, RunProcessOpts::with_timeout_secs(60), control_recv)
        .await
        .wrap_err("error creating m4s segment with gpac")?;
    Ok(())
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
    pub segment_duration: i32,
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
            "{}:profile=onDemand{}:segdur={}",
            opts.mpd_name,
            opts.base_url
                .map(|url| format!(":base={}", url))
                .unwrap_or_default(),
            opts.segment_duration
        )
        .as_str(),
    ]);
    tracing::debug!(?command);
    let child = command.spawn().context("error calling gpac")?;
    run_process(child, RunProcessOpts::with_timeout_secs(3600), control_recv)
        .await
        .wrap_err("error DASH-segmenting file with gpac")?;
    let mp4_name = format!(
        "{}_dashinit.mp4",
        input_path.file_stem().ok_or(eyre!("bad mpd filename"))?
    );
    Ok(GpacDashResult {
        mpd_path: out_dir.join(opts.mpd_name),
        mp4_path: out_dir.join(mp4_name),
    })
}
