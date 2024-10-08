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

#[derive(Debug)]
pub enum ProcessResult {
    RanToEnd(std::process::Output),
    TerminatedBySignal(std::process::Output),
    OtherError(eyre::Report),
}

pub type ProcessControlReceiver = mpsc::Receiver<ProcessControl>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    Running,
    Stopped,
}

/// Run process, waiting for messages while waiting for it to finish.
/// Returns process output even if stdout/stderr are piped to null, in which case they will be
/// empty strings.
#[cfg(target_family = "unix")]
pub async fn run_process(
    child: tokio::process::Child,
    control_recv: &mut mpsc::Receiver<ProcessControl>,
    // stdout_lines_send: mpsc::Receiver<Option<String>>,
) -> ProcessResult {
    let pid = child.id().expect("child process must not have completed");
    let (send, mut recv) = oneshot::channel();
    tokio::task::spawn(async move { send.send(child.wait_with_output().await) });
    let mut killed_by_signal = false;
    let mut state = ProcessState::Running;
    let pid = Pid::from_raw(pid.try_into().expect("pid_t is a signed 32-bit int"));
    loop {
        tokio::select! {
            // Err variant is produced when sender is dropped, which we can ignore
            Ok(result) = &mut recv => {
                match result {
                    Err(wait_err) => {
                        let r: eyre::Report = wait_err.into();
                        return ProcessResult::OtherError(r.wrap_err("error waiting for child process"));
                    }
                    Ok(output) => {
                        if killed_by_signal {
                            return ProcessResult::TerminatedBySignal(output);
                        } else {
                            return ProcessResult::RanToEnd(output);
                        }
                    }
                }
            }
            msg = control_recv.recv() => {
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
                                    return ProcessResult::OtherError(eyre::Report::from(err).wrap_err("error sending signal to process"));
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
