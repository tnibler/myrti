use std::process::Stdio;

use async_trait::async_trait;
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument};

use crate::{
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    processing::process_control::{run_process, ProcessControlReceiver, ProcessResult},
};

#[derive(thiserror::Error, Debug)]
pub enum ShakaError {
    #[error("Error starting shaka packager")]
    ErrorStarting,
    #[error("shaka packager exited by signal")]
    TerminatedBySignal,
}

#[async_trait]
pub trait ShakaPackagerTrait {
    async fn run(
        input: &Path,
        repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
        shaka_packager_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ShakaResult>;
}

#[async_trait]
pub trait ShakaPackagerWithLocalOutputTrait {
    async fn run_with_local_output(
        input: &Path,
        repr_type: RepresentationType,
        output: &Path,
        shaka_packager_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepresentationType {
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationInput {
    pub path: PathBuf,
    pub ty: RepresentationType,
    pub output_key: String,
}

pub struct ShakaPackager {}

pub struct ShakaResult {
    pub media_info_key: String,
}

#[async_trait]
impl ShakaPackagerWithLocalOutputTrait for ShakaPackager {
    #[instrument(err, name = "shaka_packager", skip(control_recv))]
    async fn run_with_local_output(
        input: &Path,
        repr_type: RepresentationType,
        output: &Path,
        shaka_packager_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<()> {
        let command_out_dir = output
            .parent()
            .expect("output path must have a parent directory");
        let command_out_filename = output
            .file_name()
            .expect("CommandOutputFile must have a filename");

        let mut command = Command::new(shaka_packager_bin_path.unwrap_or("packager".into()));
        // paths in media_info file are always written as absolute, so we cd into the output
        // directory so that the path is just the filename
        command.current_dir(command_out_dir);
        if !input.is_file() {
            return Err(eyre!("input paths for segmenting must be files"));
        }
        let stream = match repr_type {
            RepresentationType::Video => "video",
            RepresentationType::Audio => "audio",
        };
        command.arg(format!(
            "in={},stream={},output={}",
            input,
            stream,
            command_out_filename // only filename as out path because cwd ==
                                 // command_out_dir
        ));
        command.arg("--output_media_info");

        debug!(?command, "Invoking shaka-packager");
        let child = command
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .wrap_err("error calling shaka packager")?;

        match run_process(child, control_recv).await {
            ProcessResult::RanToEnd(output) if output.status.success() => Ok(()),
            ProcessResult::RanToEnd(output) => Err(eyre!(
                "shaka packager exited with an error:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )),
            ProcessResult::TerminatedBySignal(_) => Err(ShakaError::TerminatedBySignal.into()),
            ProcessResult::OtherError(err) => Err(err.wrap_err("error running shaka packager")),
        }
    }
}

#[async_trait]
impl ShakaPackagerTrait for ShakaPackager {
    #[tracing::instrument(err, skip(storage, control_recv))]
    async fn run(
        input: &Path,
        repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
        shaka_packager_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ShakaResult> {
        let mp4_out_file = storage.new_command_out_file(output_key).await?;
        let media_info_key = format!("{}.media_info", output_key);
        let media_info_out_file = storage.new_command_out_file(&media_info_key).await?;
        // FIXME: This will break with StorageProviders that use tempfiles as command outputs
        debug_assert!(media_info_out_file
            .path()
            .parent()
            .unwrap()
            .ends_with(mp4_out_file.path().parent().unwrap()));

        Self::run_with_local_output(
            input,
            repr_type,
            mp4_out_file.path(),
            shaka_packager_bin_path,
            control_recv,
        )
        .await?;

        mp4_out_file.flush_to_storage().await?;
        media_info_out_file.flush_to_storage().await?;
        Ok(ShakaResult { media_info_key })
    }
}

#[cfg(feature = "mock-commands")]
pub struct ShakaPackagerMock {}

#[cfg(feature = "mock-commands")]
#[async_trait]
impl ShakaPackagerTrait for ShakaPackagerMock {
    async fn run(
        _input: &Path,
        _repr_type: RepresentationType,
        output_key: &str,
        storage: &Storage,
        shaka_packager_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ShakaResult> {
        let mp4_out_file = storage.new_command_out_file(output_key).await?;
        let media_info_key = format!("{}.media_info", output_key);
        let media_info_out_file = storage.new_command_out_file(&media_info_key).await?;
        mp4_out_file.flush_to_storage().await?;
        media_info_out_file.flush_to_storage().await?;
        Ok(ShakaResult { media_info_key })
    }
}
