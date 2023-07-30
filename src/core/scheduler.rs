use std::path::PathBuf;

use super::{
    job::{Job, JobId},
    monitor::MonitorMessage,
};
use crate::{
    eyre::Result,
    job::{
        indexing_job::{IndexingJob, IndexingJobParams},
        thumbnail_job::ThumbnailJob,
    },
    model::AssetType,
    repository,
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
        if repository::asset::get_assets_with_missing_thumbnail(&self.pool, Some(1))
            .await
            .unwrap()
            .iter()
            .any(|asset| asset.ty == AssetType::Image)
        {
            let job = ThumbnailJob::new(Vec::new(), self.pool.clone());
            let handle = job.start();
            self.monitor_tx.send(MonitorMessage::AddJob(handle)).await;
        }
    }

    async fn queue_or_start_indexing(&mut self, params: IndexingJobParams) {
        // // always starting job, no queue yet
        // let job = params.start();
        let job = IndexingJob::new(params, self.pool.clone());
        let handle = job.start();
        self.monitor_tx.send(MonitorMessage::AddJob(handle)).await;
        // info!("id {}", id.0);
        // let mut status_rx = job.status_rx;
        // let scheduler_event_tx = self.events_tx.clone();
        // tokio::spawn(async move {
        //     // move this into a JobMonitor service that receive the status_rx of every started job,
        //     // receives events from all job, notifies the scheduler if required (e.g. Completed to
        //     // check if anything else needs to run now)
        //     // and saves the most recent status to be polled from the api.
        //     // JobMonitor also holds the cancellation tokens for the jobs
        //     while let Some(status) = status_rx.recv().await {
        //         match status {
        //             Job::Completed => {
        //                 info!("Indexing job complete");
        //                 scheduler_event_tx
        //                     .send(SchedulerEvent::JobComplete { id })
        //                     .await
        //                     .unwrap();
        //                 break;
        //             }
        //             _ => {
        //                 // todo!()
        //             }
        //         }
        //     }
        // });
    }
}
