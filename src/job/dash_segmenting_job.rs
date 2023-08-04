use eyre::{bail, Context, Report, Result};
use std::{
    sync::Arc,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, Instrument};

use crate::{
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        DataDirManager,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetId, AssetType, Video,
    },
    processing::video::{
        dash_package::{shaka_package, RepresentationInput, RepresentationType},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashSegmentingJobParams {
    pub asset_ids: Vec<AssetId>,
}

pub struct DashSegmentingJob {
    params: DashSegmentingJobParams,
    data_dir_manager: Arc<DataDirManager>,
    pool: DbPool,
}

#[derive(Debug)]
pub struct DashSegmentingJobResult {
    pub completed: Vec<AssetId>,
    pub failed: Vec<(AssetId, Report)>,
}

impl DashSegmentingJob {
    pub fn new(
        params: DashSegmentingJobParams,
        data_dir_manager: Arc<DataDirManager>,
        pool: DbPool,
    ) -> DashSegmentingJob {
        DashSegmentingJob {
            pool,
            data_dir_manager,
            params,
        }
    }

    #[instrument(name = "DashSegmentingJob", skip(self, status_tx, cancel))]
    async fn run(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> DashSegmentingJobResult {
        info!("Packaging {} videos for DASH", self.params.asset_ids.len());
        let mut failed: Vec<(AssetId, Report)> = vec![];
        let mut completed: Vec<AssetId> = vec![];
        let asset_count = self.params.asset_ids.len();
        for (index, asset_id) in self.params.asset_ids.iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            status_tx
                .send(JobProgress {
                    percent: Some((index as f32 / asset_count as f32) as i32),
                    description: format!("Processing {}", asset_id.0),
                })
                .await
                .unwrap();
            match self.process_single_asset(*asset_id).in_current_span().await {
                Ok(()) => completed.push(*asset_id),
                Err(e) => failed.push((*asset_id, e.wrap_err("error packaging video"))),
            }
        }
        DashSegmentingJobResult { completed, failed }
    }

    async fn process_single_asset(&self, asset_id: AssetId) -> Result<()> {
        let asset_base = repository::asset::get_asset_base(&self.pool, asset_id)
            .in_current_span()
            .await?;
        if asset_base.ty != AssetType::Video {
            bail!("not a video")
        }
        let asset_path = repository::asset::get_asset_path_on_disk(&self.pool, asset_id)
            .await
            .unwrap();
        let resource_dir = self
            .data_dir_manager
            .new_dash_dir(format!("{}", asset_id.0).as_str())
            .await
            .wrap_err("could not create DASH output directory")?;
        let reprs = [
            RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Video,
            },
            RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Audio,
            },
        ];
        shaka_package(&reprs, &resource_dir.path_on_disk(), "stream.mpd")
            .in_current_span()
            .await
            .wrap_err("error packaging video for DASH")?;
        let mut tx = self
            .pool
            .begin()
            .await
            .wrap_err("could not begin db transaction")?;
        let resource_dir_id =
            repository::resource_file::insert_new_resource_file(&mut tx, resource_dir)
                .in_current_span()
                .await?;
        let video_info = Video {
            dash_resource_dir: Some(resource_dir_id),
        };
        repository::asset::update_video_info(&mut tx, asset_id, &video_info).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Job for DashSegmentingJob {
    type Result = DashSegmentingJobResult;

    fn start(self) -> JobHandle {
        let (tx, rx) = mpsc::channel::<JobProgress>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move {
            let r = self.run(tx, cancel_copy).await;
            JobResultType::DashSegmenting(r)
        });
        let handle: JobHandle = JobHandle {
            progress_rx: rx,
            join_handle,
            cancel,
        };
        handle
    }
}
