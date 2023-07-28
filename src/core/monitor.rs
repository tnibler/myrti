use std::{collections::HashMap, sync::Arc};

use super::{
    job::{JobHandleType, JobId, JobStatus},
    scheduler::SchedulerEvent,
};
use eyre::eyre;
use eyre::Result;
use futures::{SinkExt, StreamExt};
use tokio::{select, sync::mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, Instrument};

#[derive(Clone)]
pub struct Monitor {
    inner: Arc<tokio::sync::Mutex<MonitorInner>>,
    statuses: Arc<tokio::sync::Mutex<HashMap<JobId, JobStatus>>>,
    scheduler_tx: mpsc::Sender<SchedulerEvent>,
}

struct MonitorInner {
    last_job_id: JobId,
    jobs: HashMap<JobId, JobInfo>,
    new_status_tx: mpsc::Sender<(JobId, mpsc::Receiver<JobStatus>)>,
}

struct JobInfo {
    id: JobId,
    cancel: CancellationToken, // join_handle: JoinHandle<T::Result>,
}

struct JobStatusWithId {
    pub id: JobId,
    pub status: JobStatus,
}

pub enum MonitorMessage {
    AddJob(JobHandleType),
}

impl Monitor {
    pub fn new(
        mut msg_rx: mpsc::Receiver<MonitorMessage>,
        scheduler_tx: mpsc::Sender<SchedulerEvent>,
        cancel: CancellationToken,
    ) -> Monitor {
        let (tx, mut rx) = mpsc::channel::<(JobId, mpsc::Receiver<JobStatus>)>(1000);
        let statuses = Arc::new(tokio::sync::Mutex::new(HashMap::<JobId, JobStatus>::new()));
        let statuses_copy = statuses.clone();
        let monitor = Monitor {
            inner: tokio::sync::Mutex::new(MonitorInner {
                last_job_id: JobId(0),
                jobs: Default::default(),
                new_status_tx: tx,
            })
            .into(),
            scheduler_tx,
            statuses: statuses_copy,
        };
        let monitor_copy = monitor.clone();
        let span = tracing::info_span!("Monitor");
        tokio::task::spawn(async move {
            let status_rxs = Vec::<futures::channel::mpsc::Receiver<JobStatusWithId>>::new();
            let mut any_status = futures::stream::select_all(status_rxs);
            loop {
                select! {
                    () = cancel.cancelled() => {
                        info!("Monitor cancelled");
                        break;
                    },
                    Some((id, mut status_rx)) = rx.recv() => {
                        debug!("New job added to monitor ({})", id);
                        let (mut status_with_id_tx, status_with_id_rx) = futures::channel::mpsc::channel::<JobStatusWithId>(1000);
                        tokio::task::spawn(async move {
                            while let Some(status) = status_rx.recv().await {
                                status_with_id_tx.send(JobStatusWithId { id, status }).await.unwrap();
                            }
                        });
                        any_status.push(status_with_id_rx);
                    },
                    Some(status_with_id) = any_status.next() => {
                        monitor_copy.on_status_received(status_with_id.id, status_with_id.status).await;
                    }
                    Some(msg) = msg_rx.recv() => {
                        match msg {
                            MonitorMessage::AddJob(job_handle) => {
                                monitor_copy.add_job(job_handle).await;
                            }
                        }
                    }
                }
            }
        }.instrument(span));
        monitor
    }

    #[instrument(skip(self))]
    async fn on_status_received(&self, job_id: JobId, status: JobStatus) {
        debug!("received status: {}, {:#?}", job_id, status);
        self.statuses.lock().await.insert(job_id, status.clone());
        match status {
            JobStatus::Complete => {
                self.scheduler_tx
                    .send(SchedulerEvent::JobComplete { id: job_id })
                    .await
                    .unwrap();
            }
            JobStatus::Failed { msg } => {
                error!("Job failed: {}", msg);
            }
            JobStatus::Cancelled => {
                info!("Job cancelled");
            }
            _ => {}
        }
    }

    pub async fn get_status(&self, id: JobId) -> Result<JobStatus> {
        self.statuses
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| eyre!("no job with this id"))
    }

    #[instrument(skip(self, handle))]
    pub async fn add_job(&self, handle: JobHandleType) -> JobId {
        let mut inner = self.inner.lock().await;
        inner.last_job_id = JobId(inner.last_job_id.0 + 1);
        let id = inner.last_job_id;
        let (job_info, status_rx) = match handle {
            JobHandleType::Indexing(h) => {
                debug!("Adding IndexingJob");
                (
                    JobInfo {
                        id,
                        cancel: h.cancel,
                    },
                    h.status_rx,
                )
            }
            JobHandleType::Thumbnail(h) => {
                debug!("Adding ThumbnailJob");
                (
                    JobInfo {
                        id,
                        cancel: h.cancel,
                    },
                    h.status_rx,
                )
            }
        };
        inner.jobs.insert(id, job_info);
        inner.new_status_tx.send((id, status_rx)).await.unwrap();
        id
    }

    #[instrument(skip(self))]
    pub async fn cancel_job(&self, id: JobId) -> Result<()> {
        debug!("Cancelling job");
        let inner = self.inner.lock().await;
        inner
            .jobs
            .get(&id)
            .ok_or(eyre!("no job with this id"))
            .and_then(|job_info| {
                job_info.cancel.cancel();
                Ok(())
            })
    }
}
