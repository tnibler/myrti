use std::{path::PathBuf};

use async_trait::async_trait;
use tokio::{sync::mpsc};
use tokio_util::sync::CancellationToken;


use crate::{
    core::job::{Job, JobHandle, JobHandleType, JobProgress, JobStatus},
    indexing,
    model::{AssetId, AssetRootDir},
    repository::{pool::DbPool},
};

pub struct IndexingJob {
    params: IndexingJobParams,
    pool: DbPool,
}

pub struct IndexingJobParams {
    pub asset_root: AssetRootDir,
    pub sub_paths: Option<Vec<PathBuf>>,
}

impl IndexingJob {
    pub fn new(params: IndexingJobParams, pool: DbPool) -> IndexingJob {
        IndexingJob { params, pool }
    }

    async fn run(
        self,
        status_tx: mpsc::Sender<JobStatus>,
        _cancel: CancellationToken,
    ) -> Vec<AssetId> {
        // let span = info_span!("IndexingJob");
        // let _enter = span.enter();
        let asset_ids: Vec<AssetId> = Vec::new();
        status_tx.send(JobStatus::Running(JobProgress {
            percent: None,
            description: format!(
                "Indexing asset root {}",
                self.params.asset_root.path.to_string_lossy()
            ),
        }));
        match indexing::index_asset_root(&self.params.asset_root, &self.pool).await {
            Ok(new_asset_ids) => {
                status_tx.send(JobStatus::Complete).await;
                return new_asset_ids;
            }
            Err(e) => {
                status_tx
                    .send(JobStatus::Failed { msg: e.to_string() })
                    .await;
                return asset_ids;
            }
        };
    }
}

#[async_trait]
impl Job for IndexingJob {
    type Result = Vec<AssetId>;

    fn start(self) -> JobHandleType {
        let (tx, rx) = mpsc::channel::<JobStatus>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move { self.run(tx, cancel_copy).await });
        let handle: JobHandle<Self> = JobHandle {
            status_rx: rx,
            join_handle,
            cancel,
        };
        JobHandleType::Indexing(handle)
    }
}
