use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    eyre::Result,
    indexing_job::{IndexingJob, IndexingJobParams, IndexingJobStatus},
    model::AssetRootDirId,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub trait Job {
    fn can_run(&self) -> bool;
    fn run();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(u64);

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
    tx: mpsc::Sender<SchedulerEvent>,
}

impl Scheduler {
    pub fn start() -> Scheduler {
        let (tx, rx) = mpsc::channel::<SchedulerEvent>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let tx_copy = tx.clone();
        let s = Scheduler {
            cancel: cancel_copy,
            tx,
        };
        tokio::spawn(async move {
            let mut si = SchedulerImpl {
                events_tx: tx_copy,
                events_rx: rx,
                cancel,
                last_job_id: JobId(0),
                cancel_tokens: HashMap::new(),
            };
            si.run().await;
        });
        s
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
    last_job_id: JobId,
    cancel_tokens: HashMap<JobId, CancellationToken>,
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
                                    let params = IndexingJobParams {
                                        asset_root_dir_id: id
                                    };
                                    self.queue_or_start_indexing(params);
                                },
                                _ => todo!()
                            }
                        },
                        SchedulerEvent::JobComplete { id }=> {
                            info!("job {} completed", id.0);
                            self.queue_jobs_if_required().await;
                            self.cancel_tokens.remove(&id);
                        },
                        SchedulerEvent::ConfigChange => todo!(),
                        SchedulerEvent::CancelJob { id } => {
                            info!("cancelling job");
                            self.cancel_tokens.get(&id).and_then(|cancel| Some(cancel.cancel()));
                        }
                    }
                }
            }
        }
    }

    async fn queue_jobs_if_required(&mut self) {
        info!("checking if any jobs need to be run...");
    }

    fn queue_or_start_indexing(&mut self, params: IndexingJobParams) {
        // always starting job, no queue yet
        let job = params.start();
        self.last_job_id = JobId(self.last_job_id.0 + 1);
        let id = self.last_job_id;
        info!("id {}", id.0);
        self.cancel_tokens.insert(id, job.cancel);
        let mut status_rx = job.status_rx;
        let scheduler_event_tx = self.events_tx.clone();
        tokio::spawn(async move {
            // move this into a JobMonitor service that receive the status_rx of every started job,
            // receives events from all job, notifies the scheduler if required (e.g. Completed to
            // check if anything else needs to run now)
            // and saves the most recent status to be polled from the api.
            // JobMonitor also holds the cancellation tokens for the jobs
            while let Some(status) = status_rx.recv().await {
                match status {
                    IndexingJobStatus::Completed => {
                        info!("Indexing job complete");
                        scheduler_event_tx
                            .send(SchedulerEvent::JobComplete { id })
                            .await
                            .unwrap();
                        break;
                    }
                    _ => {
                        // todo!()
                    }
                }
            }
        });
    }
}
