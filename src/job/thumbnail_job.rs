use std::path::PathBuf;

use async_trait::async_trait;
use eyre::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, instrument};

use crate::{
    core::job::{Job, JobHandle, JobProgress, JobResultType, JobStatus},
    model::AssetId,
    repository::{self, pool::DbPool},
    thumbnail::{generate_thumbnails, ThumbnailResult},
};

pub struct ThumbnailJob {
    params: ThumbnailJobParams,
    pool: DbPool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailJobParams {
    pub asset_ids: Vec<AssetId>,
}

#[derive(Debug)]
pub struct ThumbnailJobResult {
    pub results: Vec<ThumbnailResult>,
}

impl ThumbnailJob {
    pub fn new(params: ThumbnailJobParams, pool: DbPool) -> ThumbnailJob {
        ThumbnailJob { params, pool }
    }

    #[instrument(skip(self, status_tx))]
    async fn run(self, status_tx: mpsc::Sender<JobStatus>) -> Result<ThumbnailJobResult> {
        status_tx
            .send(JobStatus::Running(JobProgress {
                percent: None,
                description: "".to_string(),
            }))
            .await
            .unwrap();
        // TODO ignoring the parameter given to the job and just getting all assets requiring
        // thumbnails from db for now
        let assets = repository::asset::get_assets_with_missing_thumbnail(&self.pool, None)
            .await
            .unwrap();
        let result =
            generate_thumbnails(&assets, PathBuf::from("thumbnails").as_path(), &self.pool).await;
        match result {
            Ok(results) => {
                results
                    .iter()
                    .filter_map(|r| match r.result {
                        Ok(_) => None,
                        Err(ref e) => Some((r.asset_id, e)),
                    })
                    .for_each(|(id, error)| {
                        error!(
                            "Failed to generate thumbnail for asset {}: {}",
                            id,
                            error.to_string()
                        );
                    });
                status_tx.send(JobStatus::Complete).await.unwrap();
                return Ok(ThumbnailJobResult { results });
            }
            Err(e) => {
                status_tx
                    .send(JobStatus::Failed { msg: e.to_string() })
                    .await
                    .unwrap();
                return Err(e);
            }
        }
    }
}

#[async_trait]
impl Job for ThumbnailJob {
    type Result = Result<ThumbnailJobResult>;

    fn start(self) -> JobHandle {
        let (tx, rx) = mpsc::channel::<JobStatus>(1000);
        let cancel = CancellationToken::new();
        let _cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move {
            let r = self.run(tx).await;
            JobResultType::Thumbnail(r)
        });
        let handle = JobHandle {
            status_rx: rx,
            join_handle,
            cancel,
        };
        handle
    }
}
