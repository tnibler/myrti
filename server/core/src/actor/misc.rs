use std::future::Future;

use tokio::sync::mpsc;

use crate::processing::process_control::ProcessControl;

use super::simple_queue_actor::{MsgTaskControl, TaskError};

pub async fn task_loop<T>(
    mut fut: impl Future<Output = T>,
    ctl_recv: &mut mpsc::UnboundedReceiver<MsgTaskControl>,
    process_control_send: mpsc::Sender<ProcessControl>,
) -> Result<T, TaskError> {
    let mut was_cancelled = false;
    let mut fut = std::pin::pin!(fut);
    loop {
        tokio::select! {
            result = &mut fut => {
                if was_cancelled {
                    return Err(TaskError::Cancelled)
                } else {
                    return Ok(result);
                }
            }
            Some(msg) = ctl_recv.recv() => {
                if was_cancelled {
                    continue;
                }
                let (process_control, will_cancel) = match msg {
                    MsgTaskControl::Pause => (ProcessControl::Suspend, false),
                    MsgTaskControl::Resume => (ProcessControl::Resume, false),
                    MsgTaskControl::Cancel => (ProcessControl::Quit, true),
                };
                match process_control_send.send(process_control).await {
                    Ok(_) => {
                        was_cancelled = will_cancel;
                    }
                    Err(err) => {
                        return Err(TaskError::Other(eyre::Report::from(err).wrap_err("error sending process control message")));
                    }
                };
            }
        }
    }
}
