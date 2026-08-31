use std::time::Duration;

use eyre::eyre;
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessControl {
    Suspend,
    Resume,
    Quit,
    Kill,
}

#[derive(Debug, thiserror::Error)]
pub enum RunProcessError {
    #[error("Process exited with nonzero return value")]
    ExitedWithError(std::process::Output),
    #[error("Process terminated by signal")]
    TerminatedBySignal(std::process::Output),
    #[error("Process timed out ({0:?})")]
    Timeout(Duration),
    #[error(transparent)]
    Other(#[from] eyre::Report),
}

pub type ProcessControlReceiver = mpsc::Receiver<ProcessControl>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    Running,
    Stopped,
}

#[derive(Default, Clone, Copy)]
pub struct RunProcessOpts {
    pub timeout: Option<Duration>,
    _ne: (),
}

impl RunProcessOpts {
    pub fn with_timeout(d: Duration) -> Self {
        Self {
            timeout: Some(d),
            ..Default::default()
        }
    }

    pub fn with_timeout_secs(secs: u64) -> Self {
        Self {
            timeout: Some(Duration::from_secs(secs)),
            ..Default::default()
        }
    }
}

/// Run process, waiting for messages while waiting for it to finish.
/// Returns process output even if stdout/stderr are piped to null, in which case they will be
/// empty.
/// The timeout is reset whenever the child is suspended.
#[cfg(target_family = "unix")]
pub async fn run_process(
    child: tokio::process::Child,
    opts: RunProcessOpts,
    control_recv: &mut mpsc::Receiver<ProcessControl>,
    // stdout_lines_send: mpsc::Receiver<Option<String>>,
) -> Result<std::process::Output, RunProcessError> {
    use tokio::time::sleep;

    let pid = child.id().expect("child process must not have completed");
    let (send, mut recv) = oneshot::channel();
    tokio::task::spawn(async move { send.send(child.wait_with_output().await) });
    let mut killed_by_signal = false;
    let mut state = ProcessState::Running;
    let pid = Pid::from_raw(pid.try_into().expect("pid_t is a signed 32-bit int"));
    let timeout_duration = opts.timeout.unwrap_or(Duration::from_hours(48));
    let timeout = sleep(timeout_duration);
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            _ = &mut timeout, if state == ProcessState::Running => {
                tracing::debug!("Process timed out");
                _ = kill(pid, Signal::SIGKILL);
                return Err(RunProcessError::Timeout(timeout_duration));
            }
            child_msg = &mut recv => {
                tracing::trace!(?child_msg);
                match child_msg {
                    Ok(result) => {
                        match result {
                            Err(wait_err) => {
                                let r: eyre::Report = wait_err.into();
                                return Err(RunProcessError::Other(r.wrap_err("error waiting for child process")));
                            }
                            Ok(output) => {
                                tracing::trace!(?output.status, "Process exited");
                                if killed_by_signal {
                                    return Err(RunProcessError::TerminatedBySignal(output));
                                } else if output.status.success() {
                                    return Ok(output);
                                } else {
                                    return Err(RunProcessError::ExitedWithError(output));
                                }
                            }
                        }
                    }
                    Err(err) => {
                        return Err(RunProcessError::Other(eyre!("child task panicked: {}", err)));
                    }
                }
            }
            msg = control_recv.recv() => {
                tracing::trace!(?msg);
                match msg {
                    Some(msg) => {
                        if killed_by_signal {
                            // don't send signal again after killing
                            continue;
                        }
                        tracing::info!("sending signal");
                        let (signals, will_kill) = match msg {
                            ProcessControl::Suspend => ([Signal::SIGTSTP].as_slice(), false),
                            ProcessControl::Resume => ([Signal::SIGCONT].as_slice(), false),
                            ProcessControl::Quit if state == ProcessState::Stopped => ([Signal::SIGCONT, Signal::SIGTERM].as_slice(), true), // wait for it to exit on quit
                            ProcessControl::Quit => ([Signal::SIGQUIT].as_slice(), true), // wait for it to exit on quit
                            ProcessControl::Kill => ([Signal::SIGKILL].as_slice(), true),
                        };
                        for signal in signals {
                            match kill(pid, *signal) {
                                Err(err) => {
                                    tracing::error!("Error sending signal {:?} to process with PID {}", signal, pid);
                                    if will_kill {
                                        return Err(RunProcessError::Other(eyre::Report::from(err).wrap_err("error sending signal to process")));
                                    }
                                }
                                Ok(()) =>  {
                                    killed_by_signal = will_kill;
                                }
                            }
                        }
                        if msg == ProcessControl::Suspend {
                            state = ProcessState::Stopped;
                        } else if msg == ProcessControl::Resume {
                            timeout.as_mut().set(sleep(timeout_duration));
                            state = ProcessState::Running;
                        }
                    },
                    None => {
                        // should/must not happen, but we can just ignore and wait for the process
                        tracing::error!("Process control channel sender was dropped");
                    },
                }
            }
        }
    }
}
