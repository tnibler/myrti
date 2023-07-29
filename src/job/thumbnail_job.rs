use std::path::PathBuf;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use crate::{
    core::job::{Job, JobHandle, JobHandleType, JobProgress, JobStatus},
    model::AssetId,
    repository::{self, pool::DbPool},
    thumbnail::generate_thumbnails,
};

pub struct ThumbnailJob {
    asset_ids: Vec<AssetId>,
    pool: DbPool,
}

impl ThumbnailJob {
    pub fn new(asset_ids: Vec<AssetId>, pool: DbPool) -> ThumbnailJob {
        ThumbnailJob { asset_ids, pool }
    }

    #[instrument(skip(self, status_tx))]
    async fn run(self, status_tx: mpsc::Sender<JobStatus>) {
        // let assets = self.asset_ids.iter().map(|id| repository::asset::get)
        // info!("")
        status_tx
            .send(JobStatus::Running(JobProgress {
                percent: None,
                description: "".to_string(),
            }))
            .await;
        let assets = repository::asset::get_assets_with_missing_thumbnail(&self.pool, None)
            .await
            .unwrap();
        let result =
            generate_thumbnails(&assets, PathBuf::from("thumbnails").as_path(), &self.pool).await;
        match result {
            Ok(()) => {
                status_tx.send(JobStatus::Complete).await;
            }
            Err(e) => {
                status_tx
                    .send(JobStatus::Failed { msg: e.to_string() })
                    .await;
            }
        }
    }
}

#[async_trait]
impl Job for ThumbnailJob {
    type Result = ();

    fn start(self) -> JobHandleType {
        let (tx, rx) = mpsc::channel::<JobStatus>(1000);
        let cancel = CancellationToken::new();
        let _cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move { self.run(tx).await });
        let handle: JobHandle<Self> = JobHandle {
            status_rx: rx,
            join_handle,
            cancel,
        };
        JobHandleType::Thumbnail(handle)
    }
}
