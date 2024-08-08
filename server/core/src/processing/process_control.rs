use eyre::{Context, Result};
use nix::{
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone)]
pub enum ProcessControl {
    Suspend,
    Resume,
    Quit,
    Kill,
}

pub type ProcessControlReceiver = mpsc::Receiver<ProcessControl>;

/// Run process, waiting for messages while waiting for it to finish.
/// Returns process output even if stdout/stderr are piped to null, in which case they will be
/// empty strings.
#[cfg(target_family = "unix")]
pub async fn run_process(
    child: tokio::process::Child,
    control_recv: &mut mpsc::Receiver<ProcessControl>,
    // stdout_lines_send: mpsc::Receiver<Option<String>>,
) -> Result<Option<std::process::Output>> {
    let pid = child.id().expect("child process must not have completed");
    let (send, mut recv) = oneshot::channel();
    tokio::task::spawn(async move { send.send(child.wait_with_output().await) });
    loop {
        tokio::select! {
            // Err variant is produced when sender is dropped, which we can ignore
            Ok(result) = &mut recv => {
                match result {
                    Err(wait_err) => {
                        return Err(wait_err).wrap_err("error waiting for child process");
                    }
                    Ok(output) => {
                        return Ok(Some(output));
                    }
                }
            }
            msg = control_recv.recv() => {
                match msg {
                    Some(msg) => {
                        let (signal, do_return) = match msg {
                            ProcessControl::Suspend => (Signal::SIGTSTP, false),
                            ProcessControl::Resume => (Signal::SIGCONT, false),
                            ProcessControl::Quit => (Signal::SIGQUIT, false), // wait for it to exit on quit
                            ProcessControl::Kill => (Signal::SIGKILL, true),
                        };
                        let pid = Pid::from_raw(pid.try_into().expect("pid_t is a signed 32-bit int"));
                        match kill(pid, signal) {
                            Err(err) => {
                                tracing::error!("Error sending signal {:?} to process with PID {}", signal, pid);
                                if do_return {
                                    return Err(err).wrap_err("error sending signal to process")
                                }
                            }
                            Ok(()) => if do_return {
                                return Ok(None);
                            }
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
