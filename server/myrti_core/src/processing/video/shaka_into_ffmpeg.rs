use async_trait::async_trait;
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{Context, Result};
use tokio::fs::File;
use tracing::instrument;

use crate::{
    core::storage::{Storage, StorageProvider},
    processing::process_control::ProcessControlReceiver,
};

use super::{
    ffmpeg::{FFmpeg, FFmpegTrait},
    shaka::{RepresentationType, ShakaPackager, ShakaPackagerWithLocalOutputTrait, ShakaResult},
};

#[async_trait]
pub trait ShakaIntoFFmpegTrait {
    type FFmpeg: FFmpegTrait;
    async fn run(
        input: &Path,
        repr_type: RepresentationType,
        ffmpeg: &Self::FFmpeg,
        output_key: &str,
        storage: &Storage,
        shaka_bin_path: Option<&Path>,
        ffmpeg_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ShakaResult>;
}

pub struct ShakaIntoFFmpeg {}

#[async_trait]
impl ShakaIntoFFmpegTrait for ShakaIntoFFmpeg {
    type FFmpeg = FFmpeg;

    #[instrument(name = "shaka_into_ffmpeg", skip(ffmpeg, storage, control_recv))]
    async fn run(
        input: &Path,
        repr_type: RepresentationType,
        ffmpeg: &Self::FFmpeg,
        output_key: &str,
        storage: &Storage,
        shaka_bin_path: Option<&Path>,
        ffmpeg_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ShakaResult> {
        let tempdir = tempfile::tempdir().wrap_err("error creating temp directory")?;
        // we need the filename from output_key (it will be written into the media_info fiel
        // and must match the filename produced by ffmpeg hereafter).
        // this is a dirty way to achieve that:
        let out_filename = {
            let p = PathBuf::from(output_key);
            p.file_name()
                .expect("output key must have a filename")
                .to_owned()
        };
        let utf8_temp_path: camino::Utf8PathBuf = tempdir
            .path()
            .to_path_buf()
            .try_into()
            .expect("tempfile path should be utf8");
        let shaka_out_path = utf8_temp_path.join(&out_filename);
        ShakaPackager::run_with_local_output(
            input,
            repr_type,
            &shaka_out_path,
            shaka_bin_path,
            control_recv,
        )
        .await?;
        let media_info_filename = format!("{}.media_info", &out_filename);
        let media_info_key = format!("{}.media_info", output_key);
        let mut write_media_info = storage.open_write_stream(&media_info_key).await?;
        let mut read_media_info = File::open(tempdir.path().join(&media_info_filename))
            .await
            .wrap_err("error opening media_info file")?;
        tokio::io::copy(&mut read_media_info, &mut write_media_info).await?;

        ffmpeg
            .run(
                shaka_out_path.as_str(),
                output_key,
                storage,
                ffmpeg_bin_path,
                control_recv,
            )
            .await?;
        Ok(ShakaResult { media_info_key })
    }
}

#[allow(inactive-code)]
#[cfg(feature = "mock-commands")]
pub struct ShakaIntoFFmpegMock {}

#[allow(inactive-code)]
#[cfg(feature = "mock-commands")]
use super::ffmpeg::FFmpegMock;

#[allow(inactive-code)]
#[cfg(feature = "mock-commands")]
#[async_trait]
impl ShakaIntoFFmpegTrait for ShakaIntoFFmpegMock {
    type FFmpeg = FFmpegMock;

    #[instrument(name = "shaka_into_ffmpeg", skip(ffmpeg, storage, control_recv))]
    async fn run(
        input: &Path,
        repr_type: RepresentationType,
        ffmpeg: &Self::FFmpeg,
        output_key: &str,
        storage: &Storage,
        shaka_bin_path: Option<&Path>,
        ffmpeg_bin_path: Option<&Path>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ShakaResult> {
        ffmpeg
            .run(
                &PathBuf::from("MOCK_PATH"),
                output_key,
                storage,
                ffmpeg_bin_path,
                control_recv,
            )
            .await?;
        let media_info_key = format!("{}.media_info", output_key);
        Ok(ShakaResult { media_info_key })
    }
}
