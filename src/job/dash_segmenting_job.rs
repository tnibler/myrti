use eyre::{Context, Report, Result};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, Instrument};

use crate::{
    catalog::{
        encoding_target::EncodingTarget,
        operation::package_video::{
            apply_package_video, perform_side_effects_package_video, PackageVideo,
            PackageVideoWithPath,
        },
    },
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        DataDirManager,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetId, VideoAsset,
    },
};

#[derive(Debug, Clone)]
pub struct DashSegmentingJobParams {
    pub tasks: Vec<PackageVideo>,
}

pub struct DashSegmentingJob {
    params: DashSegmentingJobParams,
    data_dir_manager: Arc<DataDirManager>,
    pool: DbPool,
}

#[derive(Debug)]
pub struct DashSegmentingJobResult {
    pub completed: Vec<PackageVideo>,
    pub failed: Vec<(PackageVideo, Report)>,
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
        let mut failed: Vec<(PackageVideo, Report)> = vec![];
        let mut completed: Vec<PackageVideo> = vec![];
        for task in &self.params.tasks {
            if cancel.is_cancelled() {
                break;
            }
            match self.process_task(task.clone()).in_current_span().await {
                Ok(_) => {
                    completed.push(task.clone());
                }
                Err(err) => {
                    failed.push((task.clone(), err));
                }
            }
        }
        DashSegmentingJobResult { completed, failed }
    }

    async fn process_task(&self, package_video: PackageVideo) -> Result<()> {
        let asset: VideoAsset = repository::asset::get_asset(&self.pool, package_video.asset_id)
            .in_current_span()
            .await?
            .try_into()?;
        let resource_dir = match asset.video.dash_resource_dir {
            None => self
                .data_dir_manager
                .new_dash_dir(format!("{}", asset.base.id.0).as_str())
                .in_current_span()
                .await
                .wrap_err("error creating video resource dir")?,
            Some(resource_dir) => resource_dir,
        };
        let package_video_with_path = PackageVideoWithPath {
            output_dir: resource_dir,
            package_video,
        };
        let completed_package_video =
            perform_side_effects_package_video(&self.pool, &package_video_with_path)
                .in_current_span()
                .await
                .wrap_err("error packaging video asset")?;
        apply_package_video(&self.pool, &completed_package_video)
            .in_current_span()
            .await
            .wrap_err("error packaging video asset")?;
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
