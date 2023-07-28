use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    core::job::{Job, JobHandle, JobHandleType, JobStatus},
    model::AssetId,
    repository::pool::DbPool,
};

pub struct ThumbnailJob {
    asset_ids: Vec<AssetId>,
    pool: DbPool,
}

impl ThumbnailJob {
    pub fn new(asset_ids: Vec<AssetId>, pool: DbPool) -> ThumbnailJob {
        ThumbnailJob { asset_ids, pool }
    }

    async fn run(self, _status_tx: mpsc::Sender<JobStatus>) {}
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
