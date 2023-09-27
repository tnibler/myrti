use async_trait::async_trait;
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{Context, Result};
use tokio::fs::File;
use tracing::{instrument, Instrument};

use crate::core::storage::{Storage, StorageProvider};

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
        shaka_bin_path: Option<&str>,
        ffmpeg_bin_path: Option<&str>,
    ) -> Result<ShakaResult>;
}

pub struct ShakaIntoFFmpeg {}

#[async_trait]
impl ShakaIntoFFmpegTrait for ShakaIntoFFmpeg {
    type FFmpeg = FFmpeg;

    #[instrument(name = "shaka_into_ffmpeg", skip(ffmpeg, storage))]
    async fn run(
        input: &Path,
        repr_type: RepresentationType,
        ffmpeg: &Self::FFmpeg,
        output_key: &str,
        storage: &Storage,
        shaka_bin_path: Option<&str>,
        ffmpeg_bin_path: Option<&str>,
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
        ShakaPackager::run_with_local_output(input, repr_type, &shaka_out_path, shaka_bin_path)
            .in_current_span()
            .await?;
        let media_info_filename = format!("{}.media_info", &out_filename);
        let media_info_key = format!("{}.media_info", output_key);
        let mut write_media_info = storage
            .open_write_stream(&media_info_key)
            .in_current_span()
            .await?;
        let mut read_media_info = File::open(tempdir.path().join(&media_info_filename))
            .in_current_span()
            .await
            .wrap_err("error opening media_info file")?;
        tokio::io::copy(&mut read_media_info, &mut write_media_info)
            .in_current_span()
            .await?;

        ffmpeg
            .run(&shaka_out_path, output_key, storage, ffmpeg_bin_path)
            .in_current_span()
            .await?;
        Ok(ShakaResult { media_info_key })
    }
}

#[cfg(feature = "mock-commands")]
pub struct ShakaIntoFFmpegMock {}

#[cfg(feature = "mock-commands")]
use super::ffmpeg::FFmpegMock;

#[cfg(feature = "mock-commands")]
#[async_trait]
impl ShakaIntoFFmpegTrait for ShakaIntoFFmpegMock {
    type FFmpeg = FFmpegMock;

    #[instrument(name = "shaka_into_ffmpeg", skip(ffmpeg, storage))]
    async fn run(
        input: &Path,
        repr_type: RepresentationType,
        ffmpeg: &Self::FFmpeg,
        output_key: &str,
        storage: &Storage,
        shaka_bin_path: Option<&str>,
        ffmpeg_bin_path: Option<&str>,
    ) -> Result<ShakaResult> {
        ffmpeg
            .run(
                &PathBuf::from("MOCK_PATH"),
                output_key,
                storage,
                ffmpeg_bin_path,
            )
            .in_current_span()
            .await?;
        let media_info_key = format!("{}.media_info", output_key);
        Ok(ShakaResult { media_info_key })
    }
}
