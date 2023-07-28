use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use tokio::{select, sync::mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{info, info_span, instrument};

use crate::{
    core::job::{AJobHandle, Job, JobHandle, JobHandleType, JobStatus, TypedJobHandle},
    model::{AssetId, AssetRootDirId},
    repository::pool::DbPool,
};

pub struct IndexingJob {
    params: IndexingJobParams,
    pool: DbPool,
}

pub struct IndexingJobParams {
    pub asset_root_id: Vec<AssetRootDirId>,
    pub sub_paths: Option<Vec<PathBuf>>,
}

impl IndexingJob {
    pub fn new(params: IndexingJobParams, pool: DbPool) -> IndexingJob {
        IndexingJob { params, pool }
    }

    #[instrument(skip(self))]
    async fn run(
        self,
        status_tx: mpsc::Sender<JobStatus>,
        cancel: CancellationToken,
    ) -> Vec<AssetId> {
        // let span = info_span!("IndexingJob");
        // let _enter = span.enter();
        info!("indexing job running");
        status_tx.send(JobStatus::Running).await;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        let mut i = 0;
        tokio::task::spawn(async move {
            loop {
                select! {
                    _ = cancel.cancelled() => {
                    status_tx.send(JobStatus::Canceled).await;
                    return vec![];
                }
                    _ = interval.tick() => {
                        i += 1;
                        info!("doing indexing lalalala");
                        if i >= 20 {
                            if self.params.asset_root_id[0].0 == 3 {
                                status_tx.send(JobStatus::Failed).await;
                                return vec![];
                            }
                            status_tx.send(JobStatus::Complete).await;
                            return vec![AssetId(1), AssetId(2), AssetId(3)];
                        }
                    }
                }
            }
        })
        .await
        .unwrap()
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
        let handle: AJobHandle<Self> = AJobHandle {
            status_rx: rx,
            join_handle,
            cancel,
        };
        JobHandleType::Indexing(handle)
    }
}
