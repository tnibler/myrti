use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroUsize,
};

use myrti_data::model::FileId;
use tokio::sync::mpsc;

use crate::processing::process_control::ProcessControl;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Job was cancelled")]
    Cancelled,
    #[error("Job failed")]
    Other(#[from] eyre::Report),
}

#[derive(Clone)]
struct RunningJob {
    control: JobControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(u64);

pub trait JobProcessor: Clone {
    type Job;
    type Output;

    fn process(
        &self,
        file_id: FileId,
        job: Vec<Self::Job>,
        control: JobControlRecv,
    ) -> impl Future<Output = Result<Self::Output, JobError>> + Send;
}

pub struct BatchQueueExecutor<J, O, P> {
    max_running: usize,
    max_queued: usize,
    queue: VecDeque<(JobId, FileId, Vec<J>)>,
    processor: P,
    accepting_new: bool,

    // back up to scheduler
    result_send: mpsc::Sender<(JobId, Result<O, JobError>)>,
    running_jobs: HashMap<JobId, RunningJob>,
    next_job_id: u64,
}

impl<J, O, P> BatchQueueExecutor<J, O, P>
where
    J: Send + 'static,
    O: Send + 'static,
    P: JobProcessor<Job = J, Output = O> + Send + 'static,
{
    pub fn new(
        processor: P,
        max_running: NonZeroUsize,
        max_queued: NonZeroUsize,
        result_send: mpsc::Sender<(JobId, Result<O, JobError>)>,
    ) -> Self {
        Self {
            max_running: max_running.get(),
            max_queued: max_queued.get(),
            queue: Default::default(),
            processor,
            accepting_new: true,
            result_send,
            running_jobs: Default::default(),
            next_job_id: 0,
        }
    }

    pub fn enqueue_job(&mut self, file_id: FileId, job: J) -> Result<JobId, J> {
        if let Some((job_id, _file_id, jobs)) = self
            .queue
            .iter_mut()
            .rfind(|(_job_id, queued_file_id, _jobs)| *queued_file_id == file_id)
        {
            jobs.push(job);
            Ok(*job_id)
        } else if self.accepting_new && self.running_jobs.len() < self.max_running {
            let job_id = self.new_job_id();
            self.start_job(job_id, file_id, vec![job]);
            Ok(job_id)
        } else if self.queue.len() < self.max_queued {
            let job_id = self.new_job_id();
            self.queue.push_back((job_id, file_id, vec![job]));
            Ok(job_id)
        } else {
            Err(job)
        }
    }

    pub fn try_dequeue(&mut self) {
        let mut dequeued_count = 0;
        while self.accepting_new && self.running_jobs.len() < self.max_running {
            if let Some((job_id, file_id, jobs)) = self.queue.pop_front() {
                dequeued_count += 1;
                self.start_job(job_id, file_id, jobs);
            } else {
                break;
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
        tracing::debug!("cancelling all");
        self.accepting_new = false;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Cancel);
        }
    }

    pub fn on_job_finished(&mut self, job_id: JobId) {
        if self.running_jobs.remove(&job_id).is_none() {
            tracing::error!(?job_id, "tried removing non existing job from running_jobs");
        }
        self.try_dequeue();
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn start_job(&mut self, job_id: JobId, file_id: FileId, jobs: Vec<J>) {
        tracing::trace!(?job_id, ?file_id, "starting job");
        assert!(self.running_jobs.len() < self.max_running);
        let processor = self.processor.clone();
        let (control_send, control_recv) = new_job_control();
        let result_send = self.result_send.clone();
        self.running_jobs.insert(
            job_id,
            RunningJob {
                control: control_send,
            },
        );
        tokio::task::spawn(async move {
            let result = processor.process(file_id, jobs, control_recv).await;
            let _ = result_send.send((job_id, result)).await;
        });
    }

    fn new_job_id(&mut self) -> JobId {
        self.next_job_id += 1;
        JobId(self.next_job_id)
    }
}

fn new_job_control() -> (JobControl, JobControlRecv) {
    let (send, recv) = mpsc::unbounded_channel();
    (JobControl { send }, JobControlRecv { recv })
}

#[derive(Clone)]
struct JobControl {
    send: mpsc::UnboundedSender<JobControlMsg>,
}

pub struct JobControlRecv {
    pub recv: mpsc::UnboundedReceiver<JobControlMsg>,
}

#[derive(Debug, Clone)]
pub enum JobControlMsg {
    Pause,
    Resume,
    Cancel,
}

pub async fn run_process_loop<T>(
    fut: impl Future<Output = T>,
    ctl_recv: &mut JobControlRecv,
    process_control_send: mpsc::Sender<ProcessControl>,
) -> Result<T, JobError> {
    let mut was_cancelled = false;
    let mut fut = std::pin::pin!(fut);
    loop {
        tokio::select! {
            result = &mut fut => {
                if was_cancelled {
                    return Err(JobError::Cancelled)
                } else {
                    return Ok(result);
                }
            }
            Some(msg) = ctl_recv.recv.recv() => {
                if was_cancelled {
                    continue;
                }
                let (process_control, will_cancel) = match msg {
                    JobControlMsg::Pause => (ProcessControl::Suspend, false),
                    JobControlMsg::Resume => (ProcessControl::Resume, false),
                    JobControlMsg::Cancel => (ProcessControl::Quit, true),
                };
                match process_control_send.send(process_control).await {
                    Ok(_) => {
                        was_cancelled = will_cancel;
                    }
                    Err(err) => {
                        return Err(JobError::Other(eyre::Report::from(err).wrap_err("error sending process control message")));
                    }
                };
            }
        }
    }
}
