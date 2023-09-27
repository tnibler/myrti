use async_trait::async_trait;
use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Result};
use tracing::Instrument;

use crate::{
    core::storage::{CommandOutputFile, StorageCommandOutput},
    processing::image::ffmpeg_snapshot::ffmpeg_snapshot,
};

use super::{
    vips_wrapper::{self, VipsThumbnailParams},
    OutDimension,
};

pub struct ThumbnailParams<'a> {
    pub in_path: PathBuf,
    pub outputs: Vec<&'a CommandOutputFile>,
    pub out_dimension: OutDimension,
}

#[async_trait]
pub trait GenerateThumbnailTrait {
    async fn generate_thumbnail<'a>(params: ThumbnailParams<'a>) -> Result<()>;
    async fn generate_video_thumbnail<'a>(params: ThumbnailParams<'a>) -> Result<()>;
}

pub struct GenerateThumbnail {}

pub struct GenerateThumbnailMock {}

#[async_trait]
impl GenerateThumbnailTrait for GenerateThumbnail {
    async fn generate_thumbnail<'a>(params: ThumbnailParams<'a>) -> Result<()> {
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
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<()>>();
        rayon::spawn(move || {
            let res = vips_wrapper::generate_thumbnail(vips_params);
            tx.send(res).unwrap();
        });
        rx.await
            .wrap_err("error generating thumbnail with libvips")?
            .wrap_err("error generating thumbnail with libvips")
    }

    async fn generate_video_thumbnail<'a>(params: ThumbnailParams<'a>) -> Result<()> {
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
        ffmpeg_snapshot(&params.in_path, &utf8_snapshot_path, Some("ffmpeg"))
            .in_current_span()
            .await?;
        Self::generate_thumbnail(ThumbnailParams {
            in_path: utf8_snapshot_path,
            ..params
        })
        .in_current_span()
        .await?;
        snapshot_path
            .persist(PathBuf::from("/tmp/snap.webp"))
            .unwrap();
        Ok(())
    }
}

#[async_trait]
impl GenerateThumbnailTrait for GenerateThumbnailMock {
    async fn generate_thumbnail<'a>(_params: ThumbnailParams<'a>) -> Result<()> {
        Ok(())
    }

    async fn generate_video_thumbnail<'a>(_params: ThumbnailParams<'a>) -> Result<()> {
        Ok(())
    }
}
