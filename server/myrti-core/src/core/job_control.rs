use tokio::sync::mpsc;

use crate::processing::process_control::ProcessControl;

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Job was cancelled")]
    Cancelled,
    #[error("Job failed")]
    Other(#[from] eyre::Report),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

#[derive(Clone)]
pub struct JobControl {
    pub send: mpsc::UnboundedSender<JobControlMsg>,
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

pub fn new_job_control() -> (JobControl, JobControlRecv) {
    let (send, recv) = mpsc::unbounded_channel();
    (JobControl { send }, JobControlRecv { recv })
}

pub(super) async fn run_process_loop<T>(
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
