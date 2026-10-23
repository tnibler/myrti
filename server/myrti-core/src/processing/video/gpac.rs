use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{Context, Result, eyre};
use tokio::process::Command;

use crate::processing::process_control::{ProcessControlReceiver, RunProcessOpts, run_process};

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

#[tracing::instrument(skip(control_recv), level = "debug")]
pub async fn run_dasher(
    input_path: &Path,
    out_dir: &Path,
    opts: DasherOptions<'_>,
    gpac_bin_path: (&Path, &[String]),
    control_recv: &mut ProcessControlReceiver,
) -> Result<GpacDashResult> {
    let (gpac_path, args) = gpac_bin_path;
    let mut command = Command::new(gpac_path);
    command.current_dir(out_dir).args(args).args([
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
