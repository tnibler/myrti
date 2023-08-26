use std::{path::PathBuf, sync::Arc};

use super::{
    job::{Job, JobId, JobResultType},
    monitor::MonitorMessage,
    DataDirManager,
};
use crate::{
    catalog::{operation::package_video::PackageVideo, rules},
    core::job::JobType,
    eyre::Result,
    job::{
        dash_segmenting_job::{DashSegmentingJob, DashSegmentingJobParams},
        indexing_job::{IndexingJob, IndexingJobParams},
        thumbnail_job::{ThumbnailJob, ThumbnailJobParams},
    },
};
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, instrument};

#[derive(Debug)]
pub enum SchedulerMessage {
    Timer,
    FileSystemChange { changed_files: Vec<PathBuf> },
    UserRequest(UserRequest),
    JobComplete { id: JobId, result: JobResultType },
    JobFailed { id: JobId },
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
    #[instrument(name = "event_loop", skip(self))]
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
                            }
                        },
                        SchedulerMessage::JobComplete {id, result }=> {
                            self.on_job_complete(id, result).await;
                        },
                        SchedulerMessage::JobFailed { id: _ } => {

                        }
                        SchedulerMessage::ConfigChange => todo!(),
                    }
                }
            }
        }
    }

    async fn on_job_complete(&self, _job_id: JobId, result: JobResultType) {
        match result {
            JobResultType::Indexing(_) => {
                self.thumbnail_if_required().await;
                self.dash_package_if_required().await;
            }
            JobResultType::Thumbnail(_) => {}
            JobResultType::DashSegmenting(_) => {}
        }
    }

    async fn thumbnail_if_required(&self) {
        let thumbnails_to_create = rules::thumbnails_to_create(&self.pool).await.unwrap();
        if !thumbnails_to_create.is_empty() {
            let params = ThumbnailJobParams {
                thumbnails: thumbnails_to_create,
            };
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

    async fn dash_package_if_required(&self) {
        let videos_to_package: Vec<PackageVideo> =
            rules::video_packaging_due(&self.pool).await.unwrap();
        debug!(?videos_to_package);
        if !videos_to_package.is_empty() {
            let params = DashSegmentingJobParams {
                tasks: videos_to_package,
            };
            let job = DashSegmentingJob::new(
                params.clone(),
                self.data_dir_manager.clone(),
                self.pool.clone(),
            );
            let handle = job.start();
            self.monitor_tx
                .send(MonitorMessage::AddJob {
                    handle,
                    ty: JobType::DashSegmenting { params },
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
