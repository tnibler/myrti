use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, info_span, Instrument};

use crate::model::AssetRootDirId;

pub struct IndexingJobParams {
    pub asset_root_dir_id: AssetRootDirId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexingJobStatus {
    NotStarted,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl IndexingJobParams {
    pub fn start(self) -> IndexingJob {
        let span = info_span!("IndexingJob");
        let _enter = span.enter();
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let (status_tx, status_rx) = mpsc::channel::<IndexingJobStatus>(1000);
        let _ = status_tx.send(IndexingJobStatus::NotStarted);
        tokio::spawn(
            async move {
                let _ = status_tx.send(IndexingJobStatus::Running);
                // while !cancel.is_cancelled() {
                for _ in 1..8 {
                    if cancel.is_cancelled() {
                        let _ = status_tx.send(IndexingJobStatus::Cancelled).await.unwrap();
                        info!("IndexingJob cancelled");
                        return;
                    }
                    info!("IndexingJob is running");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                let _ = status_tx.send(IndexingJobStatus::Completed).await.unwrap();
                return;
                // }
            }
            .in_current_span(),
        );
        IndexingJob {
            cancel: cancel_copy,
            status_rx,
        }
    }
}

pub struct IndexingJob {
    pub cancel: CancellationToken,
    pub status_rx: mpsc::Receiver<IndexingJobStatus>,
}

impl IndexingJob {
    // pub fn cancel(&mut self) {
    //     self.cancel.cancel();
    // }
}
