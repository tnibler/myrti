use async_trait::async_trait;
use eyre::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    core::job::{Job, JobHandle, JobProgress, JobResultType, JobStatus},
    indexing,
    model::{AssetId, AssetRootDir, AssetRootDirId},
    repository::pool::DbPool,
};

pub struct IndexingJob {
    params: IndexingJobParams,
    pool: DbPool,
}

// This type doesn't have to be this way as an IndexingJob only indexes one
// AssetRootDir for now but that can change
#[derive(Debug)]
pub struct IndexingJobResult {
    pub new_asset_ids: Vec<AssetId>,
    pub failed: Vec<(AssetRootDirId, eyre::Report)>,
}

#[derive(Debug)]
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
    ) -> Result<IndexingJobResult> {
        // let span = info_span!("IndexingJob");
        // let _enter = span.enter();
        let asset_ids: Vec<AssetId> = Vec::new();
        status_tx
            .send(JobStatus::Running(JobProgress {
                percent: None,
                description: format!(
                    "Indexing asset root {}",
                    self.params.asset_root.path.to_string_lossy()
                ),
            }))
            .await
            .unwrap();
        let mut failed = Vec::<(AssetRootDirId, eyre::Report)>::new();
        let mut new_asset_ids = Vec::<AssetId>::new();
        match indexing::index_asset_root(&self.params.asset_root, &self.pool).await {
            Ok(mut asset_ids) => {
                status_tx.send(JobStatus::Complete).await.unwrap();
                new_asset_ids.append(&mut asset_ids);
            }
            Err(e) => {
                status_tx
                    .send(JobStatus::Failed { msg: e.to_string() })
                    .await
                    .unwrap();
                failed.push((self.params.asset_root.id, e));
            }
        };
        Ok(IndexingJobResult {
            failed,
            new_asset_ids,
        })
    }
}

#[async_trait]
impl Job for IndexingJob {
    type Result = Result<IndexingJobResult>;

    fn start(self) -> JobHandle {
        let (tx, rx) = mpsc::channel::<JobStatus>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move {
            let r = self.run(tx, cancel_copy).await;
            JobResultType::Indexing(r)
        });
        let handle: JobHandle = JobHandle {
            status_rx: rx,
            join_handle,
            cancel,
        };
        handle
    }
}
