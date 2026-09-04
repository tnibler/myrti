use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use eyre::{Context, Result};
use tokio::sync::mpsc;

use myrti_data::db::DbPool;

use crate::{
    catalog::operation::package_video::{PackageVideo, do_package_video},
    config::Config,
    core::{
        job_control::{
            JobControl, JobControlMsg, JobControlRecv, JobError, JobId, new_job_control,
            run_process_loop,
        },
        storage::Storage,
    },
};

#[derive(Clone)]
struct RunningJob {
    control: JobControl,
    job: VideoJob,
}

#[derive(Debug, Clone)]
pub(super) enum VideoJob {
    PackageVideo(PackageVideo),
}

pub(super) type VideoProcessingMsg = (JobId, Result<Result<()>, JobError>);

#[derive(Clone)]
pub(super) struct VideoJobProcessor {
    max_running: usize,
    max_queued: usize,
    queue: VecDeque<(JobId, VideoJob)>,
    accepting_new: bool,
    // back up to scheduler
    result_send: Option<mpsc::Sender<VideoProcessingMsg>>,
    running_jobs: HashMap<JobId, RunningJob>,
    next_job_id: u64,

    db_pool: DbPool,
    storage: Storage,
    config: Arc<Mutex<Config>>,
}

impl VideoJobProcessor {
    pub fn new(
        max_running: NonZeroUsize,
        max_queued: NonZeroUsize,
        result_send: mpsc::Sender<VideoProcessingMsg>,
        db_pool: DbPool,
        storage: Storage,
        config: Arc<Mutex<Config>>,
    ) -> Self {
        Self {
            max_running: max_running.get(),
            max_queued: max_queued.get(),
            queue: Default::default(),
            accepting_new: true,
            result_send: Some(result_send),
            running_jobs: Default::default(),
            next_job_id: 0,
            db_pool,
            storage,
            config,
        }
    }

    pub fn enqueue_job(&mut self, job: VideoJob) -> Result<JobId, VideoJob> {
        if !self.accepting_new || self.queue.len() >= self.max_queued {
            return Err(job);
        }
        let job_id = self.new_job_id();

        if self.running_jobs.len() < self.max_running {
            self.start_job(job_id, job);
        } else {
            self.queue.push_back((job_id, job));
        }
        Ok(job_id)
    }

    pub fn try_dequeue(&mut self) {
        let mut dequeued_count = 0;
        while self.accepting_new && self.running_jobs.len() < self.max_running {
            match self.queue.pop_front() {
                Some((job_id, job)) => {
                    dequeued_count += 1;
                    self.start_job(job_id, job);
                }
                None => break,
            }
        }
        let remaining_queued = self.queued();
        tracing::trace!(dequeued_count, remaining_queued, "try_dequeue");
    }

    pub fn pause_all(&mut self) {
        tracing::debug!("pausing all");
        self.accepting_new = false;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Pause);
        }
    }

    pub fn resume_all(&mut self) {
        tracing::debug!("resuming all");
        self.accepting_new = true;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Resume);
        }
    }

    pub fn cancel_all(&mut self) {
        let n_jobs = self.running_jobs.len();
        tracing::debug!(n_jobs, "cancelling all");
        self.accepting_new = false;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Cancel);
        }
    }

    pub fn shutdown(&mut self) {
        self.cancel_all();
        self.result_send = None;
    }

    pub fn on_job_finished(&mut self, job_id: JobId) {
        self.running_jobs
            .remove(&job_id)
            .expect("tried removing non existing job from running_jobs");
        self.try_dequeue();
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn start_job(&mut self, job_id: JobId, job: VideoJob) {
        assert!(self.running_jobs.len() < self.max_running);
        tracing::trace!(?job_id, "starting job");
        let (control_send, control_recv) = new_job_control();
        let result_send = self.result_send.clone().expect("must not be shut down");
        self.running_jobs.insert(
            job_id,
            RunningJob {
                control: control_send,
                job: job.clone(),
            },
        );
        let db_pool = self.db_pool.clone();
        let storage = self.storage.clone();
        let config = self.config.clone();
        tokio::task::spawn(async move {
            let result = process(db_pool, storage, config, job, control_recv).await;
            let _ = result_send.send((job_id, result)).await;
        });
    }

    fn new_job_id(&mut self) -> JobId {
        self.next_job_id += 1;
        JobId(self.next_job_id)
    }
}

async fn process(
    db_pool: DbPool,
    storage: Storage,
    config: Arc<Mutex<Config>>,
    job: VideoJob,
    mut control_recv: JobControlRecv,
) -> Result<Result<()>, JobError> {
    match job {
        VideoJob::PackageVideo(package_op) => {
            tracing::trace!(?package_op, "packaging video");
            let (process_control_send, process_control_recv) = tokio::sync::mpsc::channel(1);
            let bin_paths = config.lock().unwrap().bin_paths.clone();
            let result_fut = do_package_video(
                &db_pool,
                &storage,
                package_op.clone(),
                bin_paths.as_ref(),
                process_control_recv,
            );
            run_process_loop(result_fut, &mut control_recv, process_control_send)
                .await
                .wrap_err("error running video packaging job")??;
        }
    }
    Ok(Ok(()))
}
