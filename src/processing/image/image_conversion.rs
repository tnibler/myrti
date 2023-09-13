use std::path::PathBuf;

use async_trait::async_trait;
use eyre::{Context, Result};

use crate::{
    catalog::image_conversion_target::ImageConversionTarget,
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
};

use super::vips_wrapper;

#[async_trait]
pub trait ConvertImageTrait {
    async fn convert_image(
        path: PathBuf,
        target: ImageConversionTarget,
        output_key: &str,
        storage: &Storage,
    ) -> Result<()>;
}

pub struct ConvertImage {}

#[async_trait]
impl ConvertImageTrait for ConvertImage {
    async fn convert_image(
        path: PathBuf,
        target: ImageConversionTarget,
        output_key: &str,
        storage: &Storage,
    ) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        let out_path = command_out_file.path().to_owned();
        let (tx, rx) = tokio::sync::oneshot::channel();
        rayon::spawn(move || {
            let result = vips_wrapper::convert_image(path.as_path(), out_path.as_path(), &target);
            tx.send(result).unwrap();
        });
        rx.await
            .wrap_err("error in image conversion task")?
            .wrap_err("error converting image")?;
        command_out_file.flush_to_storage().await?;
        Ok(())
    }
}

#[cfg(feature = "mock-commands")]
pub struct ConvertImageMock {}

#[cfg(feature = "mock-commands")]
#[async_trait]
impl ConvertImageTrait for ConvertImageMock {
    async fn convert_image(
        path: PathBuf,
        target: ImageConversionTarget,
        output_key: &str,
        storage: &Storage,
    ) -> Result<()> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        command_out_file.flush_to_storage().await?;
        Ok(())
    }
}
