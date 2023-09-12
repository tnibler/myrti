use std::path::PathBuf;

use async_trait::async_trait;
use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::{debug, instrument, Instrument};

use crate::core::storage::{Storage, StorageCommandOutput, StorageProvider};

#[async_trait]
pub trait MpdGeneratorTrait {
    async fn run(
        media_info_keys: impl Iterator<Item = &str> + Send,
        output_key: &str,
        storage: &Storage,
    ) -> Result<()>;
}

pub struct MpdGenerator {}

#[async_trait]
impl MpdGeneratorTrait for MpdGenerator {
    #[instrument(name = "mpd_generator", skip(storage, media_info_keys))]
    async fn run(
        media_info_keys: impl Iterator<Item = &str> + Send,
        output_key: &str,
        storage: &Storage,
    ) -> Result<()> {
        enum MediaInfoPath {
            Tempfile(tempfile::TempPath),
            Local(PathBuf),
        }
        let mut paths: Vec<MediaInfoPath> = Vec::default();
        for key in media_info_keys {
            let mip = if let Some(local_path) = storage.local_path(key).await? {
                MediaInfoPath::Local(local_path)
            } else {
                let tempfile = tempfile::Builder::new()
                    .suffix(".media_info")
                    .tempfile()
                    .wrap_err("error creating temp file")?;
                let temp_path = tempfile.into_temp_path();
                let mut read = storage.open_read_stream(key).await?;
                let mut write = tokio::fs::File::open(&temp_path).await?;
                tokio::io::copy(&mut read, &mut write).await?;
                MediaInfoPath::Tempfile(temp_path)
            };
            paths.push(mip);
        }
        let command_out_file = storage.new_command_out_file(output_key).await?;
        let input_str = paths
            .iter()
            .map(|p| match p {
                MediaInfoPath::Local(path) => path.to_str().unwrap(),
                MediaInfoPath::Tempfile(temp_path) => temp_path.to_str().unwrap(),
            })
            .collect::<Vec<_>>()
            .join(",");
        let mpd_out_str = command_out_file.path().to_str().unwrap();
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
        command_out_file.flush_to_storage().await?;
        // FIXME mpd_generator just skips input media_info files that fail to open/don't exist
        // but carries on and exits with 0 so this check doesn't do anything
        if !result.success() {
            return Err(eyre!("mpd_generator exited with error"));
        }
        Ok(())
    }
}

#[cfg(feature = "mock-commands")]
pub struct MpdGeneratorMock {}

#[cfg(feature = "mock-commands")]
#[async_trait]
impl MpdGeneratorTrait for MpdGenerator {
    #[instrument(name = "mpd_generator", skip(storage, media_info_keys))]
    async fn run(
        media_info_keys: impl Iterator<Item = &str> + Send,
        output_key: &str,
        storage: &Storage,
    ) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        command_out_file.flush_to_storage().await?;
        Ok(())
    }
}
