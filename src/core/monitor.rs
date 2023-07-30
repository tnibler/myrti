use std::{cell::Cell, collections::HashMap, fmt::Display, sync::Arc};

use super::{
    job::{JobHandle, JobId, JobResultType, JobStatus, JobType},
    scheduler::SchedulerMessage,
};
use eyre::eyre;
use eyre::Result;
use futures::{SinkExt, StreamExt};
use tokio::{
    select,
    sync::mpsc,
    task::{JoinError, JoinHandle},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span, instrument, warn, Instrument};

#[derive(Clone)]
pub struct Monitor {
    inner: Arc<tokio::sync::Mutex<MonitorInner>>,
    statuses: Arc<tokio::sync::Mutex<HashMap<JobId, JobStatus>>>,
    scheduler_tx: mpsc::Sender<SchedulerMessage>,
    job_result_tx: mpsc::Sender<Result<JobResultType, JoinError>>,
    new_status_tx: mpsc::Sender<NewJobToWatch>,
}

struct MonitorInner {
    last_job_id: JobId,
    jobs: HashMap<JobId, JobInfo>,
}

struct JobInfo {
    id: JobId,
    ty: JobType,
    cancel: CancellationToken,
}

struct JobStatusWithId {
    pub id: JobId,
    pub status: JobStatus,
}

#[derive(Debug, Clone)]
pub struct AJobInfo {
    pub id: JobId,
    pub ty: JobType,
    pub status: Option<JobStatus>,
}

pub enum MonitorMessage {
    AddJob { handle: JobHandle, ty: JobType },
}

pub struct NewJobToWatch {
    pub id: JobId,
    pub status_rx: mpsc::Receiver<JobStatus>,
}

impl Display for MonitorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MonitorMessage::AddJob { handle, ty } => write!(f, "AddJob({})", ty),
        }
    }
}

impl Monitor {
    pub fn new(
        mut msg_rx: mpsc::Receiver<MonitorMessage>,
        scheduler_tx: mpsc::Sender<SchedulerMessage>,
        cancel: CancellationToken,
    ) -> Monitor {
        let (job_result_tx, mut job_result_rx) =
            mpsc::channel::<Result<JobResultType, JoinError>>(1000);
        let (add_job_tx, mut add_job_rx) = mpsc::channel::<NewJobToWatch>(1000);
        let statuses = Arc::new(tokio::sync::Mutex::new(HashMap::<JobId, JobStatus>::new()));
        let statuses_copy = statuses.clone();
        let monitor = Monitor {
            new_status_tx: add_job_tx,
            job_result_tx,
            inner: tokio::sync::Mutex::new(MonitorInner {
                last_job_id: JobId(0),
                jobs: Default::default(),
            })
            .into(),
            scheduler_tx,
            statuses: statuses_copy,
        };
        let monitor_copy = monitor.clone();
        let event_loop_span = tracing::info_span!("Monitor event loop");
        tokio::task::spawn(async move {
            let status_rxs = Vec::<futures::channel::mpsc::Receiver<JobStatusWithId>>::new();
            let mut any_status = futures::stream::select_all(status_rxs);
            loop {
                select! {
                    () = cancel.cancelled() => {
                        info!("Monitor cancelled");
                        break;
                    },
                    // New job added with its corresponding channel to read incoming status updates
                    // from
                    Some(NewJobToWatch { id, mut status_rx }) = add_job_rx.recv() => {
                        debug!(job_id=%id, "New job added to monitor");
                        let (mut status_with_id_tx, status_with_id_rx) = futures::channel::mpsc::channel::<JobStatusWithId>(1000);
                        tokio::task::spawn(async move {
                            while let Some(status) = status_rx.recv().await {
                                status_with_id_tx.send(JobStatusWithId { id, status }).await.unwrap();
                            }
                        });
                        any_status.push(status_with_id_rx);
                    },
                    // Some job sent a new status
                    Some(status_with_id) = any_status.next() => {
                        monitor_copy.on_status_received(status_with_id.id, status_with_id.status).in_current_span().await;
                    }
                    Some(msg) = msg_rx.recv() => {
                        debug!(%msg, "Received message");
                        match msg {
                            MonitorMessage::AddJob{ handle, ty } => {
                                monitor_copy.add_job(handle, ty).await;
                            }
                        }
                    }
                    Some(job_result) = job_result_rx.recv() => {
                        info!(?job_result, "Received result from job");
                    }
                }
            }
        }.instrument(event_loop_span));
        monitor
    }

    #[instrument(name = "Status received", skip(self), level = "info")]
    async fn on_status_received(&self, job_id: JobId, status: JobStatus) {
        self.statuses.lock().await.insert(job_id, status.clone());
        match status {
            JobStatus::Complete => {
                self.scheduler_tx
                    .send(SchedulerMessage::JobComplete { id: job_id })
                    .in_current_span()
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

    #[instrument(name = "Add job", skip(self, handle))]
    pub async fn add_job(&self, handle: JobHandle, ty: JobType) -> JobId {
        let mut inner = self.inner.lock().await;
        inner.last_job_id = JobId(inner.last_job_id.0 + 1);
        let id = inner.last_job_id;
        let job_info = JobInfo {
            id,
            ty,
            cancel: handle.cancel,
        };
        let status_rx = handle.status_rx;
        inner.jobs.insert(id, job_info);
        self.new_status_tx
            .send(NewJobToWatch { id, status_rx })
            .await
            .unwrap();
        let job_result_tx = self.job_result_tx.clone();
        // wait for job to return a result or panic
        tokio::task::spawn(
            async move {
                let join_result: Result<JobResultType, JoinError> =
                    handle.join_handle.in_current_span().await;
                match join_result {
                    Ok(ref job_result) => {
                        debug!(result_type=%job_result, "Received result");
                    }
                    Err(ref join_error) => {
                        debug!(%join_error, "Error joining job");
                    }
                };
                job_result_tx.send(join_result).await.unwrap();
            }
            .instrument(info_span!("Waiting for job result")),
        );
        id
    }

    #[instrument(name = "Cancel job", skip(self))]
    pub async fn cancel_job(&self, id: JobId) -> Result<()> {
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

    #[instrument(name = "Get all jobs", skip(self))]
    pub async fn get_all_jobs(&self) -> Result<Vec<AJobInfo>> {
        let inner = self.inner.lock().in_current_span().await;
        let mut infos: Vec<AJobInfo> = Vec::new();
        for job in inner.jobs.values() {
            let status = self
                .statuses
                .lock()
                .in_current_span()
                .await
                .get(&job.id)
                .cloned();
            if status.is_none() {
                warn!(job_id=%job.id, "No status for job");
            }
            infos.push(AJobInfo {
                id: job.id,
                ty: job.ty.clone(),
                status,
            })
        }
        Ok(infos)
    }
}
