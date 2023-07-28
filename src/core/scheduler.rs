use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{
    job::{Job, JobId},
    monitor::MonitorMessage,
};
use crate::{
    eyre::Result,
    job::indexing_job::{IndexingJob, IndexingJobParams},
    model::AssetRootDirId,
};
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub enum SchedulerEvent {
    Timer,
    FileSystemChange { changed_files: Vec<PathBuf> },
    UserRequest(UserRequest),
    JobComplete { id: JobId },
    ConfigChange,
    CancelJob { id: JobId },
}

pub enum UserRequest {
    ReindexFullRoot { id: AssetRootDirId },
    ReindexPartialRoot { id: AssetRootDirId, path: PathBuf },
    ReindexAll,
}

#[derive(Clone)]
pub struct Scheduler {
    cancel: CancellationToken,
    pub tx: mpsc::Sender<SchedulerEvent>,
}

impl Scheduler {
    pub fn start(monitor_tx: mpsc::Sender<MonitorMessage>, pool: SqlitePool) -> Scheduler {
        let (tx, rx) = mpsc::channel::<SchedulerEvent>(1000);
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

    pub async fn send(&self, msg: SchedulerEvent) -> Result<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}

struct SchedulerImpl {
    pub events_tx: mpsc::Sender<SchedulerEvent>,
    pub events_rx: mpsc::Receiver<SchedulerEvent>,
    pub cancel: CancellationToken,
    pool: SqlitePool,
    monitor_tx: mpsc::Sender<MonitorMessage>,
}

impl SchedulerImpl {
    async fn run(&mut self) {
        info!("Scheduler starting");
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Scheduler cancelled");
                    break;
                }
                Some(event) = self.events_rx.recv() => {
                    match event {
                        SchedulerEvent::Timer => todo!(),
                        SchedulerEvent::FileSystemChange { changed_files } => todo!(),
                        SchedulerEvent::UserRequest(request) => {
                            match request {
                                UserRequest::ReindexFullRoot { id } => {
                                        info!("scheduler received event ReindexFullRoot");
                                    let params = IndexingJobParams { asset_root_id: vec![id], sub_paths: None };
                                    self.queue_or_start_indexing(params).await;
                                },
                                _ => todo!()
                            }
                        },
                        SchedulerEvent::JobComplete { id }=> {
                            info!("job {} completed", id.0);
                            self.queue_jobs_if_required().await;
                        },
                        SchedulerEvent::ConfigChange => todo!(),
                        // TODO cancelling should be done on the monitor directly so we get the
                        // result synchrounously
                        SchedulerEvent::CancelJob { id } => {
                            info!("cancelling job");
                            // self.monitor.cancel_job(id);
                        }
                    }
                }
            }
        }
    }

    async fn queue_jobs_if_required(&mut self) {
        info!("checking if any jobs need to be run...");
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
