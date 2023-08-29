use eyre::{Context, Report, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{instrument, Instrument};

use crate::{
    catalog::operation::package_video::{
        apply_package_video, perform_side_effects_package_video, PackageVideo,
    },
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        storage::Storage,
    },
    model::repository::pool::DbPool,
};

#[derive(Debug, Clone)]
pub struct DashSegmentingJobParams {
    pub tasks: Vec<PackageVideo>,
}

pub struct DashSegmentingJob {
    params: DashSegmentingJobParams,
    storage: Storage,
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
        storage: Storage,
        pool: DbPool,
    ) -> DashSegmentingJob {
        DashSegmentingJob {
            params,
            storage,
            pool,
        }
    }

    #[instrument(name = "DashSegmentingJob", skip(self, _status_tx, cancel))]
    async fn run(
        self,
        _status_tx: mpsc::Sender<JobProgress>,
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
        let completed_package_video =
            perform_side_effects_package_video(&self.pool, &self.storage, &package_video)
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
