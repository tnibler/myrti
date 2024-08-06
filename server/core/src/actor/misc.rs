use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::processing::process_control::ProcessControl;

use super::simple_queue_actor::MsgTaskControl;

pub fn pipe_task_ctl_to_process_ctl(
    mut recv: mpsc::UnboundedReceiver<MsgTaskControl>,
    send: mpsc::Sender<ProcessControl>,
    cancel: CancellationToken,
) {
    tokio::task::spawn(
        async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        break;
                    }
                    Some(msg) = recv.recv() => {
                        let process_control = match msg {
                            MsgTaskControl::Pause => ProcessControl::Suspend,
                            MsgTaskControl::Resume => ProcessControl::Resume,
                            MsgTaskControl::Cancel => ProcessControl::Quit,
                        };
                        send.send(process_control).await.expect("TODO");
                    }
                }
            }
        }
        .in_current_span(),
    );
}
