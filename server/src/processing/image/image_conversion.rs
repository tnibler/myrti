use std::path::PathBuf;

use async_trait::async_trait;
use eyre::{Context, Result};

use crate::{
    catalog::image_conversion_target::ImageConversionTarget,
    core::storage::{Storage, StorageCommandOutput, StorageProvider},
    model::Size,
    processing,
};

#[async_trait]
pub trait ConvertImageTrait {
    /// returns Size if image was scaled during conversion
    async fn convert_image(
        path: PathBuf,
        target: ImageConversionTarget,
        output_key: &str,
        storage: &Storage,
    ) -> Result<Option<Size>>;
}

pub struct ConvertImage {}

#[async_trait]
impl ConvertImageTrait for ConvertImage {
    async fn convert_image(
        path: PathBuf,
        target: ImageConversionTarget,
        output_key: &str,
        storage: &Storage,
    ) -> Result<Option<Size>> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        let out_path = command_out_file.path().to_owned();
        let (tx, rx) = tokio::sync::oneshot::channel();
        rayon::spawn(move || {
            let res = processing::image::convert_image(&path, &out_path, &target);
            tx.send(res).expect("receiver thread should not have died");
        });
        let size = rx
            .await
            .wrap_err("error in image conversion task")?
            .wrap_err("error converting image")?
            .map(|processing_size| Size {
                width: processing_size.width as i64,
                height: processing_size.height as i64,
            });
        command_out_file.flush_to_storage().await?;
        Ok(size)
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
    ) -> Result<Option<Size>> {
        let command_out_file = storage.new_command_out_file(output_key).await?;
        command_out_file.flush_to_storage().await?;
        Ok(None)
    }
}
