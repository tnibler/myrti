use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    future::Future,
};

use eyre::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

#[derive(Debug)]
pub enum MsgFrom<T: Debug> {
    ActivityChange {
        is_running: bool,
        active_tasks: usize,
        queued_tasks: usize,
    },
    DroppedMessage,
    TaskResult(Result<T, TaskError>),
}

#[derive(Debug)]
pub enum MsgTo<T: Debug> {
    PauseAll,
    ResumeAll,
    Shutdown,
    DoTask(T),
}

#[derive(Debug, Clone)]
pub enum MsgTaskControl {
    Pause,
    Resume,
    Cancel,
}

#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("Task was cancelled")]
    Cancelled,
    #[error("Task failed")]
    Other(#[from] eyre::Report),
}

#[derive(Clone)]
pub struct QueuedActorHandle<T: Debug + Send + Sync> {
    send: mpsc::UnboundedSender<MsgTo<T>>,
}

impl<Task: Debug + Send + Sync + 'static> QueuedActorHandle<Task> {
    pub fn new<TaskResult: Debug + Send + Sync + 'static, A: Actor<Task, TaskResult> + 'static>(
        actor: A,
        send_from_us: mpsc::UnboundedSender<MsgFrom<TaskResult>>,
        did_shutdown_send: oneshot::Sender<()>,
        opts: ActorOptions,
        span: tracing::Span,
    ) -> Self {
        let (send, recv) = mpsc::unbounded_channel::<MsgTo<Task>>();
        tokio::task::spawn(
            async move {
                run_actor(recv, send_from_us, did_shutdown_send, actor, opts).await;
            }
            .instrument(span),
        );
        QueuedActorHandle { send }
    }

    pub fn msg_pause_all(&self) -> Result<()> {
        self.send.send(MsgTo::PauseAll)?;
        Ok(())
    }

    pub fn msg_resume_all(&self) -> Result<()> {
        self.send.send(MsgTo::ResumeAll)?;
        Ok(())
    }

    pub fn msg_shutdown(&self) -> Result<()> {
        self.send.send(MsgTo::Shutdown)?;
        Ok(())
    }

    pub fn msg_do_task(&self, msg: Task) -> Result<()> {
        self.send.send(MsgTo::DoTask(msg))?;
        Ok(())
    }
}

pub trait Actor<Task: Debug + Send + Sync, TaskResult: Debug + Send + Sync>: Send + Sync {
    fn run_task(
        &mut self,
        msg: Task,
        result_send: mpsc::UnboundedSender<(TaskId, Result<TaskResult, TaskError>)>,
        task_id: TaskId,
        ctl_recv: mpsc::UnboundedReceiver<MsgTaskControl>,
    ) -> impl Future<Output = ()> + Send;
}

struct Runner<
    Task: Debug + Send + Sync,
    TaskResult: Debug + Send + Sync,
    A: Actor<Task, TaskResult>,
> {
    opts: ActorOptions,
    is_running: bool,
    active_tasks: usize,
    queue: VecDeque<Task>,
    send_from_us: mpsc::UnboundedSender<MsgFrom<TaskResult>>,
    actor_result_send: mpsc::UnboundedSender<(TaskId, Result<TaskResult, TaskError>)>,
    actor: A,
    next_task_id: TaskId,
    task_ctl_sends: HashMap<TaskId, mpsc::UnboundedSender<MsgTaskControl>>,

    did_shutdown_send: Option<oneshot::Sender<()>>,
    waiting_for_shutdown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(usize);

const SEND_ERROR_MESSAGE: &str = "Receiver held by scheduler, which must be alive";
impl<Task: Debug + Send + Sync, TaskResult: Debug + Send + Sync, A: Actor<Task, TaskResult>>
    Runner<Task, TaskResult, A>
{
    #[tracing::instrument(skip(self))]
    async fn pause_all(&mut self) {
        if self.is_running {
            tracing::debug!("pausing");
            self.task_ctl_sends.values().for_each(|send| {
                send.send(MsgTaskControl::Pause).expect("TODO");
            });
            self.is_running = false;
            self.signal_activity_change();
        }
    }

    #[tracing::instrument(skip(self))]
    async fn resume_all(&mut self) {
        if !self.is_running {
            self.is_running = true;
            self.task_ctl_sends.values().for_each(|send| {
                send.send(MsgTaskControl::Resume).expect("TODO");
            });
            self.dequeue_work_if_available().await;
            self.signal_activity_change();
        }
    }

    async fn shutdown(&mut self) {
        if !self.waiting_for_shutdown {
            self.is_running = false;
            tracing::info!("starting shutdown");
            self.waiting_for_shutdown = true;
            self.task_ctl_sends.values().for_each(|send| {
                send.send(MsgTaskControl::Cancel).expect("TODO");
            });
            self.signal_activity_change();
        }
    }

    #[tracing::instrument(skip(self))]
    async fn dequeue_work_if_available(&mut self) {
        while self.active_tasks < self.opts.max_tasks {
            if let Some(msg) = self.queue.pop_front() {
                tracing::debug!(?msg, "dequeuing message");
                self.start_task(msg).await;
            } else {
                break;
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn start_task(&mut self, msg: Task) {
        assert!(self.is_running);
        assert!(
            self.active_tasks < self.opts.max_tasks,
            "too many tasks: self.active_tasks >= MAX_TASKS"
        );
        let (ctl_send, ctl_recv) = mpsc::unbounded_channel::<MsgTaskControl>();
        let id = self.next_task_id;
        assert!(
            self.task_ctl_sends.insert(id, ctl_send).is_none(),
            "Next TaskId already in map"
        );
        self.next_task_id.0 += 1;
        self.actor
            .run_task(msg, self.actor_result_send.clone(), id, ctl_recv)
            .await;
        self.active_tasks += 1;
        self.signal_activity_change();
    }

    #[tracing::instrument(skip(self))]
    fn signal_activity_change(&mut self) {
        if self.active_tasks == 0 && self.waiting_for_shutdown {
            self.is_running = false;
            self.waiting_for_shutdown = false;
            tracing::info!("no more active tasks doing shutdown");
            self.did_shutdown_send
                .take()
                .expect("TODO shutdown can only be called once")
                .send(())
                .expect("receiver must be alive");
        }
        self.send_from_us
            .send(MsgFrom::ActivityChange {
                is_running: self.is_running,
                active_tasks: self.active_tasks,
                queued_tasks: self.queue.len(),
            })
            .expect(SEND_ERROR_MESSAGE);
    }

    async fn on_task_finished(&mut self, task_id: TaskId, result: Result<TaskResult, TaskError>) {
        assert!(
            self.task_ctl_sends.remove(&task_id).is_some(),
            "TaskId of finished task not in map"
        );
        self.send_from_us
            .send(MsgFrom::TaskResult(result))
            .expect(SEND_ERROR_MESSAGE);
        self.active_tasks -= 1;
        self.signal_activity_change();
        if self.is_running {
            self.dequeue_work_if_available().await;
        }
    }

    async fn on_task_received(&mut self, task: Task) {
        if self.is_running && self.active_tasks < self.opts.max_tasks {
            self.start_task(task).await;
        } else if self.queue.len() < self.opts.max_queue_size {
            self.queue.push_back(task);
            self.signal_activity_change();
        } else {
            self.send_from_us
                .send(MsgFrom::DroppedMessage)
                .expect(SEND_ERROR_MESSAGE);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActorOptions {
    pub max_tasks: usize,
    pub max_queue_size: usize,
}

#[tracing::instrument(skip_all)]
pub async fn run_actor<
    Task: Debug + Send + Sync + 'static,
    TaskResult: Debug + Send + Sync + 'static,
>(
    mut actor_recv: mpsc::UnboundedReceiver<MsgTo<Task>>,
    send: mpsc::UnboundedSender<MsgFrom<TaskResult>>,
    did_shutdown_send: oneshot::Sender<()>,
    actor: impl Actor<Task, TaskResult>,
    opts: ActorOptions,
) {
    let (actor_result_send, mut actor_result_recv) = mpsc::unbounded_channel();
    let mut runner: Runner<Task, TaskResult, _> = Runner {
        opts,
        is_running: true,
        active_tasks: 0,
        queue: Default::default(),
        send_from_us: send,
        actor_result_send,
        actor,
        next_task_id: TaskId(0),
        task_ctl_sends: Default::default(),
        did_shutdown_send: Some(did_shutdown_send),
        waiting_for_shutdown: false,
    };
    loop {
        tokio::select! {
            Some(msg) = actor_recv.recv() => {
                match msg {
                    MsgTo::PauseAll => {
                        runner.pause_all().await;
                    }
                    MsgTo::ResumeAll => {
                        runner.resume_all().await;
                    }
                    MsgTo::Shutdown => {
                        runner.shutdown().await;
                    }
                    MsgTo::DoTask(task) => {
                        runner.on_task_received(task).await;
                    }
                }
            }
            Some((task_id, result)) = actor_result_recv.recv() => {
                runner.on_task_finished(task_id, result).await;
            }
        }
    }
}
