use std::{path::PathBuf, sync::Arc};

use super::{
    job::{Job, JobId},
    monitor::MonitorMessage,
    DataDirManager,
};
use crate::{
    core::job::JobType,
    eyre::Result,
    job::{
        indexing_job::{IndexingJob, IndexingJobParams},
        thumbnail_job::{ThumbnailJob, ThumbnailJobParams},
    },
    model::repository,
    model::{AssetId, AssetType},
};
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, info_span, instrument, Instrument};

#[derive(Debug)]
pub enum SchedulerMessage {
    Timer,
    FileSystemChange { changed_files: Vec<PathBuf> },
    UserRequest(UserRequest),
    JobComplete { id: JobId },
    ConfigChange,
}

#[derive(Debug)]
pub enum UserRequest {
    ReindexAssetRoots { params: IndexingJobParams },
}

#[derive(Clone)]
pub struct Scheduler {
    cancel: CancellationToken,
    pub tx: mpsc::Sender<SchedulerMessage>,
}

impl Scheduler {
    pub fn start(monitor_tx: mpsc::Sender<MonitorMessage>, pool: SqlitePool) -> Scheduler {
        let (tx, rx) = mpsc::channel::<SchedulerMessage>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let tx_copy = tx.clone();
        tokio::spawn(async move {
            let mut si = SchedulerImpl {
                events_tx: tx_copy,
                events_rx: rx,
                cancel,
                data_dir_manager: Arc::new(DataDirManager::new(pool.clone())),
                pool,
                monitor_tx,
            };
            si.run().await;
        });
        Scheduler {
            cancel: cancel_copy,
            tx,
        }
    }

    pub async fn send(&self, msg: SchedulerMessage) -> Result<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}

struct SchedulerImpl {
    pub events_tx: mpsc::Sender<SchedulerMessage>,
    pub events_rx: mpsc::Receiver<SchedulerMessage>,
    pub cancel: CancellationToken,
    pool: SqlitePool,
    monitor_tx: mpsc::Sender<MonitorMessage>,
    data_dir_manager: Arc<DataDirManager>,
}

impl SchedulerImpl {
    #[instrument(name = "Scheduler event loop", skip(self))]
    async fn run(&mut self) {
        info!("Scheduler starting");
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Scheduler cancelled");
                    break;
                }
                Some(message) = self.events_rx.recv() => {
                    debug!(?message, "Received message");
                    match message {
                        SchedulerMessage::Timer => todo!(),
                        SchedulerMessage::FileSystemChange { changed_files: _ } => todo!(),
                        SchedulerMessage::UserRequest(request) => {
                            match request {
                                UserRequest::ReindexAssetRoots { params } => {
                                    self.queue_or_start_indexing(params).await;
                                },
                                _ => todo!()
                            }
                        },
                        SchedulerMessage::JobComplete { id }=> {
                            self.queue_jobs_if_required().await;
                        },
                        SchedulerMessage::ConfigChange => todo!(),
                    }
                }
            }
        }
    }

    async fn queue_jobs_if_required(&mut self) {
        info!("checking if any jobs need to be run...");
        if !repository::asset::get_assets_with_missing_thumbnail(&self.pool, Some(1))
            .await
            .unwrap()
            .is_empty()
        {
            let asset_ids: Vec<AssetId> =
                repository::asset::get_assets_with_missing_thumbnail(&self.pool, None)
                    .await
                    .unwrap()
                    .iter()
                    .map(|asset| asset.id)
                    .collect();
            let params = ThumbnailJobParams { asset_ids };
            let job = ThumbnailJob::new(
                params.clone(),
                self.data_dir_manager.clone(),
                self.pool.clone(),
            );
            let handle = job.start();
            self.monitor_tx
                .send(MonitorMessage::AddJob {
                    handle,
                    ty: JobType::Thumbnail { params },
                })
                .await
                .unwrap();
        }
    }

    async fn queue_or_start_indexing(&mut self, params: IndexingJobParams) {
        // // always starting job, no queue yet
        let job = IndexingJob::new(params.clone(), self.pool.clone());
        let handle = job.start();
        self.monitor_tx
            .send(MonitorMessage::AddJob {
                handle,
                ty: JobType::Indexing { params },
            })
            .await
            .unwrap();
    }
}
