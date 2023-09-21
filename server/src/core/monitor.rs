use super::{
    job::{JobHandle, JobId, JobProgress, JobResultType, JobStatus, JobType},
    scheduler::SchedulerMessage,
};
use eyre::eyre;
use eyre::Result;
use futures::{SinkExt, StreamExt};
use std::{collections::HashMap, fmt::Display, sync::Arc};
use tokio::{select, sync::mpsc, task::JoinError};
use tokio_util::sync::CancellationToken;
use tracing::{debug, debug_span, error, info, instrument, warn, Instrument};

#[derive(Clone)]
pub struct Monitor {
    inner: Arc<tokio::sync::Mutex<MonitorInner>>,
    statuses: Arc<tokio::sync::Mutex<HashMap<JobId, JobStatus>>>,
    scheduler_tx: mpsc::Sender<SchedulerMessage>,
    job_result_tx: mpsc::Sender<(JobId, Result<JobResultType, JoinError>)>,
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
    pub progress: JobProgress,
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
    pub progress_rx: mpsc::Receiver<JobProgress>,
}

impl Display for MonitorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MonitorMessage::AddJob { handle: _, ty } => write!(f, "AddJob({})", ty),
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
            mpsc::channel::<(JobId, Result<JobResultType, JoinError>)>(1000);
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
                    Some(NewJobToWatch { id, mut progress_rx }) = add_job_rx.recv() => {
                        debug!(job_id=%id, "New job added to monitor");
                        let (mut progress_with_id_tx, progress_with_id_rx) = futures::channel::mpsc::channel::<JobStatusWithId>(1000);
                        tokio::task::spawn(async move {
                            while let Some(progress) = progress_rx.recv().await {
                                progress_with_id_tx.send(JobStatusWithId { id, progress }).await.unwrap();
                            }
                        });
                        any_status.push(progress_with_id_rx);
                    },
                    // Some job sent a new status
                    Some(status_with_id) = any_status.next() => {
                        monitor_copy.on_status_received(status_with_id.id, status_with_id.progress).in_current_span().await;
                    }
                    Some(msg) = msg_rx.recv() => {
                        match msg {
                            MonitorMessage::AddJob{ handle, ty } => {
                                let id = monitor_copy.add_job(handle, ty.clone()).await;
                                monitor_copy.scheduler_tx.send(SchedulerMessage::JobRegisteredWithMonitor { id, job_type: ty }).await.unwrap();
                            }
                        }
                    }
                    Some((job_id, job_result)) = job_result_rx.recv() => {
                        debug!(?job_result, "Received job result");
                            monitor_copy.on_result_received(job_id, job_result).await;
                    }
                }
            }
        });
        monitor
    }

    #[instrument(skip(self))]
    async fn on_status_received(&self, job_id: JobId, progress: JobProgress) {
        let mut statuses = self.statuses.lock().await;
        match statuses.get(&job_id) {
            None | Some(JobStatus::NotStarted) | Some(JobStatus::Running(_)) => {
                statuses.insert(job_id, JobStatus::Running(progress));
            }
            Some(status) => {
                error!(immediate=true, %job_id, ?status, "Must not receive progress updates for job with this status");
            }
        }
    }

    async fn on_result_received(&self, job_id: JobId, result: Result<JobResultType, JoinError>) {
        match result {
            Ok(job_result) => {
                match job_result {
                    JobResultType::Indexing(ref indexing_result) => {
                        let status = if indexing_result.failed.is_empty() {
                            warn!("IndexingJob failures");
                            JobStatus::CompleteWithErrors
                        } else {
                            JobStatus::Complete
                        };
                        self.set_status(job_id, status).await;
                    }
                    JobResultType::Thumbnail(ref thumbnail_results) => {
                        let status = match thumbnail_results {
                            Err(e) => {
                                warn!("ThumbnailJob failed (like, completely)");
                                JobStatus::Failed { msg: e.to_string() }
                            }
                            Ok(r) if !r.failed.is_empty() => {
                                warn!("ThumbnailJob failures");
                                JobStatus::Failed {
                                    msg: format!("some thumbnail tasks failed: {:?}", r.failed),
                                }
                            }
                            Ok(_results) => JobStatus::Complete,
                        };
                        self.set_status(job_id, status).await;
                    }
                    JobResultType::VideoPackaging(ref segmenting_results) => {
                        let status = if segmenting_results.failed.is_empty() {
                            JobStatus::Complete
                        } else if segmenting_results.completed.is_empty() {
                            warn!("DashSegmenting failures (only failures in fact)");
                            JobStatus::Failed {
                                msg: format!("all tasks failed"),
                            }
                        } else {
                            warn!("DashSegmenting failures");
                            JobStatus::CompleteWithErrors
                        };
                        self.set_status(job_id, status).await;
                    }
                }
                self.scheduler_tx
                    .send(SchedulerMessage::JobComplete {
                        id: job_id,
                        result: job_result,
                    })
                    .in_current_span()
                    .await
                    .unwrap();
            }
            Err(join_error) => {
                self.set_status(
                    job_id,
                    JobStatus::Failed {
                        msg: join_error.to_string(),
                    },
                )
                .await;
                self.scheduler_tx
                    .send(SchedulerMessage::JobFailed { id: job_id })
                    .in_current_span()
                    .await
                    .unwrap();
            }
        }
    }

    async fn set_status(&self, job_id: JobId, status: JobStatus) {
        // status must not be updated if the job has ended
        if let Some(old_status) = self.statuses.lock().await.get(&job_id) {
            debug_assert!(!matches!(
                old_status,
                JobStatus::Complete
                    | JobStatus::CompleteWithErrors
                    | JobStatus::Cancelled
                    | JobStatus::Failed { msg: _ },
            ));
        }
        self.statuses.lock().await.insert(job_id, status);
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
    pub async fn add_job(&self, handle: JobHandle, ty: JobType) -> JobId {
        let mut inner = self.inner.lock().await;
        inner.last_job_id = JobId(inner.last_job_id.0 + 1);
        let id = inner.last_job_id;
        let job_info = JobInfo {
            id,
            ty,
            cancel: handle.cancel,
        };
        let progress_rx = handle.progress_rx;
        inner.jobs.insert(id, job_info);
        self.new_status_tx
            .send(NewJobToWatch { id, progress_rx })
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
                        debug!(immediate=true, result_type=%job_result, "Received result");
                    }
                    Err(ref join_error) => {
                        debug!(immediate=true, %join_error, "Error joining job");
                    }
                };
                job_result_tx.send((id, join_result)).await.unwrap();
            }
            .instrument(debug_span!("Waiting for job result")),
        );
        id
    }

    #[instrument(skip(self))]
    pub async fn cancel_job(&self, id: JobId) -> Result<()> {
        let inner = self.inner.lock().await;
        inner
            .jobs
            .get(&id)
            .ok_or(eyre!("no job with this id"))
            .and_then(|job_info| {
                job_info.cancel.cancel();
                Ok(())
            })?;
        drop(inner);
        self.statuses.lock().await.insert(id, JobStatus::Cancelled);
        Ok(())
    }

    #[instrument(skip(self))]
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
                warn!(immediate=true, job_id=%job.id, "No status for job");
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
