use async_trait::async_trait;
use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Result};

use crate::{
    core::storage::{CommandOutputFile, StorageCommandOutput},
    model::Size,
    processing::{
        image::ffmpeg_snapshot::ffmpeg_snapshot, process_control::ProcessControlReceiver,
    },
};

use super::{
    vips_wrapper::{self, VipsThumbnailParams},
    OutDimension,
};

#[derive(Debug)]
pub struct ThumbnailParams<'a> {
    pub in_path: PathBuf,
    pub outputs: Vec<&'a CommandOutputFile>,
    pub out_dimension: OutDimension,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailResult {
    pub actual_size: Size,
}

#[async_trait]
pub trait GenerateThumbnailTrait {
    async fn generate_thumbnail<'a>(params: ThumbnailParams<'a>) -> Result<ThumbnailResult>;
    async fn generate_video_thumbnail<'a>(
        params: ThumbnailParams<'a>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ThumbnailResult>;
}

pub struct GenerateThumbnail {}

pub struct GenerateThumbnailMock {}

#[async_trait]
impl GenerateThumbnailTrait for GenerateThumbnail {
    #[tracing::instrument]
    async fn generate_thumbnail<'a>(params: ThumbnailParams<'a>) -> Result<ThumbnailResult> {
        let out_paths: Vec<PathBuf> = params
            .outputs
            .iter()
            .map(|f| f.path().to_path_buf())
            .collect();
        let vips_params = VipsThumbnailParams {
            in_path: params.in_path,
            out_paths,
            out_dimension: params.out_dimension,
        };
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<_>>();
        rayon::spawn(move || {
            let res = vips_wrapper::generate_thumbnail(vips_params);
            tx.send(res).unwrap();
        });
        let vips_result = rx
            .await
            .wrap_err("error generating thumbnail with libvips")?
            .wrap_err("error generating thumbnail with libvips")?;
        Ok(ThumbnailResult {
            actual_size: Size {
                width: vips_result.actual_size.width,
                height: vips_result.actual_size.height,
            },
        })
    }

    #[tracing::instrument(skip(control_recv))]
    async fn generate_video_thumbnail<'a>(
        params: ThumbnailParams<'a>,
        control_recv: &mut ProcessControlReceiver,
    ) -> Result<ThumbnailResult> {
        let snapshot_path = tempfile::Builder::new()
            .prefix("snap")
            .suffix(".webp")
            .tempfile()
            .wrap_err("could not create temp file")?
            .into_temp_path();
        let utf8_snapshot_path: camino::Utf8PathBuf = snapshot_path
            .to_path_buf()
            .try_into()
            .expect("tempfile paths should be UTF8");
        // fixme ffmpeg path should come from config
        ffmpeg_snapshot(
            &params.in_path,
            &utf8_snapshot_path,
            Some("ffmpeg"),
            control_recv,
        )
        .await
        .wrap_err("error taking video snapshot")?;
        Self::generate_thumbnail(ThumbnailParams {
            in_path: utf8_snapshot_path,
            ..params
        })
        .await
    }
}

#[async_trait]
impl GenerateThumbnailTrait for GenerateThumbnailMock {
    #[tracing::instrument]
    async fn generate_thumbnail<'a>(_params: ThumbnailParams<'a>) -> Result<ThumbnailResult> {
        Ok(ThumbnailResult {
            actual_size: Size {
                width: 400,
                height: 400,
            },
        })
    }

    #[tracing::instrument]
    async fn generate_video_thumbnail<'a>(
        _params: ThumbnailParams<'a>,
        _control_recv: &mut ProcessControlReceiver,
    ) -> Result<ThumbnailResult> {
        Ok(ThumbnailResult {
            actual_size: Size {
                width: 400,
                height: 400,
            },
        })
    }
}
