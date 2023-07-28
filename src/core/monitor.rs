use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::{
    job::{AJobHandle, Job, JobHandle, JobHandleType, JobId, JobStatus, TypedJobHandle},
    scheduler::SchedulerEvent,
};
use eyre::eyre;
use eyre::Result;
use futures::{stream::FuturesUnordered, SinkExt, StreamExt};
use tokio::{select, sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;

pub struct Monitor {
    last_job_id: JobId,
    scheduler_tx: mpsc::Sender<SchedulerEvent>,
    jobs: HashMap<JobId, JobInfo>,
    new_status_tx: mpsc::Sender<(JobId, mpsc::Receiver<JobStatus>)>,
    statuses: Arc<Mutex<HashMap<u64, JobStatus>>>,
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
    ) -> Arc<tokio::sync::Mutex<Monitor>> {
        let (tx, mut rx) = mpsc::channel::<(JobId, mpsc::Receiver<JobStatus>)>(1000);
        let statuses = Arc::new(Mutex::new(HashMap::<u64, JobStatus>::new()));
        let statuses_copy = Arc::clone(&statuses);
        tokio::task::spawn(async move {
            let status_rxs = Vec::<futures::channel::mpsc::Receiver<JobStatusWithId>>::new();
            let mut any_status = futures::stream::select_all(status_rxs);
            loop {
                select! {
                    () = cancel.cancelled() => {
                        info!("monitor cancelled")
                    },
                    Some((id, mut status_rx)) = rx.recv() => {
                        info!("new job {} added to monitor", id.0);
                        let (mut status_with_id_tx, status_with_id_rx) = futures::channel::mpsc::channel::<JobStatusWithId>(1000);
                        tokio::task::spawn(async move {
                            while let Some(status) = status_rx.recv().await {
                                status_with_id_tx.send(JobStatusWithId { id, status }).await.unwrap();
                            }
                        });
                        any_status.push(status_with_id_rx);
                    },
                    Some(status_with_id) = any_status.next() => {
                        info!("received status: {}, {:#?}", status_with_id.id.0, status_with_id.status);
                        info!("lock");
                        let mut m = statuses.lock().unwrap();
                        m.insert(status_with_id.id.0, status_with_id.status);
                        info!("{} statuses", m.len());
                        info!("unlock");
                    }
                }
            }
        });
        let monitor = Arc::new(tokio::sync::Mutex::new(Monitor {
            last_job_id: JobId(0),
            scheduler_tx,
            jobs: Default::default(),
            new_status_tx: tx,
            statuses: statuses_copy,
        }));
        let monitor_copy = monitor.clone();
        tokio::task::spawn(async move {
            while let Some(msg) = msg_rx.recv().await {
                match msg {
                    MonitorMessage::AddJob(job_handle) => {
                        let mut m = monitor.lock().await;
                        m.add_job(job_handle).await;
                    }
                }
            }
        });
        monitor_copy
    }

    pub fn get_status(&self, id: JobId) -> Result<JobStatus> {
        let m = self.statuses.lock().unwrap();
        info!("{} statuses", m.len());
        m.get(&id.0)
            .cloned()
            .ok_or_else(|| eyre!("no job with this id"))
    }

    pub async fn add_job(&mut self, handle: JobHandleType) -> JobId {
        self.last_job_id = JobId(self.last_job_id.0 + 1);
        let id = self.last_job_id;
        let (job_info, status_rx) = match handle {
            JobHandleType::Indexing(h) => (
                JobInfo {
                    id,
                    cancel: h.cancel,
                },
                h.status_rx,
            ),
            JobHandleType::Thumbnail(h) => (
                JobInfo {
                    id,
                    cancel: h.cancel,
                },
                h.status_rx,
            ),
        };
        self.jobs.insert(id, job_info);
        self.new_status_tx.send((id, status_rx)).await.unwrap();
        id
    }

    pub fn cancel_job(&self, id: JobId) -> Result<()> {
        self.jobs
            .get(&id)
            .ok_or(eyre!("no job with this id"))
            .and_then(|job_info| {
                job_info.cancel.cancel();
                Ok(())
            })
    }
}
