use std::collections::VecDeque;

use eyre::{Report, Result};
use tokio::sync::mpsc;

use crate::{
    catalog::operation::package_video::{
        apply_package_video, perform_side_effects_package_video, CompletedPackageVideo,
        PackageVideo,
    },
    config,
    core::storage::Storage,
    model::repository::db::DbPool,
};

#[derive(Debug)]
pub enum MsgFromVideoPackaging {
    ActivityChange {
        running: usize,
        queued: usize,
    },
    DroppedMessage,
    PackagingComplete(PackageVideo),
    PackagingError {
        package_video: PackageVideo,
        report: Report,
    },
}

#[derive(Clone)]
pub struct VideoPackagingActorHandle {
    send: mpsc::UnboundedSender<ToVideoPackagingMsg>,
}

#[derive(Debug, Clone)]
enum ToVideoPackagingMsg {
    Pause,
    Resume,
    DoTask(DoTaskMsg),
}

#[derive(Debug, Clone)]
enum DoTaskMsg {
    PackageVideo(PackageVideo),
}

type TaskResult = Result<CompletedPackageVideo>;

impl VideoPackagingActorHandle {
    pub fn new(
        db_pool: DbPool,
        storage: Storage,
        config: config::Config,
        send_from_us: mpsc::UnboundedSender<MsgFromVideoPackaging>,
    ) -> Self {
        let (send, recv) = mpsc::unbounded_channel::<ToVideoPackagingMsg>();
        let (task_result_send, task_result_recv) = mpsc::unbounded_channel::<TaskResult>();
        let actor = VideoPackagingActor {
            db_pool,
            storage,
            config,
            send_from_us,
            task_result_send,
        };
        tokio::spawn(run_video_packaging_actor(recv, task_result_recv, actor));
        Self { send }
    }

    pub fn msg_package_video(&self, msg: PackageVideo) -> Result<()> {
        self.send
            .send(ToVideoPackagingMsg::DoTask(DoTaskMsg::PackageVideo(msg)))?;
        Ok(())
    }
}

struct VideoPackagingActor {
    pub db_pool: DbPool,
    pub storage: Storage,
    pub config: config::Config,
    pub send_from_us: mpsc::UnboundedSender<MsgFromVideoPackaging>,
    task_result_send: mpsc::UnboundedSender<TaskResult>,
}

const MAX_TASKS: usize = 4;
const MAX_QUEUE_SIZE: usize = 10;
async fn run_video_packaging_actor(
    mut actor_recv: mpsc::UnboundedReceiver<ToVideoPackagingMsg>,
    mut task_result_recv: mpsc::UnboundedReceiver<TaskResult>,
    actor: VideoPackagingActor,
) {
    let mut is_running = true;
    let mut running_tasks: usize = 0;
    let mut queue: VecDeque<DoTaskMsg> = Default::default();
    loop {
        tokio::select! {
            Some(msg) = actor_recv.recv() => {
                match msg {
                    ToVideoPackagingMsg::Pause => {
                        is_running = false;
                    }
                    ToVideoPackagingMsg::Resume => {
                        is_running = true;
                    }
                    ToVideoPackagingMsg::DoTask(task) => {
                        if is_running && running_tasks < MAX_TASKS {
                            tracing::debug!(?task, "received msg, processing immediately");
                            running_tasks += 1;
                            let _ = actor.send_from_us.send(MsgFromVideoPackaging::ActivityChange {
                                running: running_tasks,
                                queued: queue.len(),
                            });
                            actor.process_message(task).await;
                        } else if queue.len() < MAX_QUEUE_SIZE {
                            tracing::debug!("received msg, queuing it");
                            queue.push_back(task);
                            let _ = actor.send_from_us.send(MsgFromVideoPackaging::ActivityChange {
                                running: running_tasks,
                                queued: queue.len(),
                            });
                        } else {
                            let _ = actor.send_from_us.send(MsgFromVideoPackaging::DroppedMessage);
                            tracing::debug!("received msg, queue full, dropping");
                        }
                    }
                }
            }
            Some(task_result) = task_result_recv.recv() => {
                running_tasks -= 1;
                if !is_running || (queue.is_empty() && running_tasks == 0) {
                    tracing::debug!("no more messages, idle");
                } else if let Some(msg) = queue.pop_front() {
                    tracing::debug!("dequeuing message");
                    actor.process_message(msg).await;
                    running_tasks += 1;
                }
                let _ = actor.send_from_us.send(MsgFromVideoPackaging::ActivityChange {
                    running: running_tasks,
                    queued: queue.len(),
                });
                let handling_result = actor.on_video_packaging_result(task_result).await;
                if let Err(err) = handling_result {
                    // TODO: do something
                    tracing::error!(?err, "error applying operation");
                }
            }
        }
    }
}

impl VideoPackagingActor {
    #[tracing::instrument(skip(self))]
    async fn process_message(&self, msg: DoTaskMsg) {
        match msg {
            DoTaskMsg::PackageVideo(package_video) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                let bin_paths = self.config.bin_paths.clone();
                let result_send = self.task_result_send.clone();
                tokio::task::spawn(async move {
                    let result = perform_side_effects_package_video(
                        db_pool,
                        &storage,
                        &package_video,
                        bin_paths.as_ref(),
                    )
                    .await;
                    result_send.send(result).expect("TODO this error must be handled, since the work won't be written to db which is relevant");
                });
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn on_video_packaging_result(&self, result: TaskResult) -> Result<()> {
        match result {
            Ok(completed_package_video) => {
                let mut conn = self.db_pool.get().await?;
                apply_package_video(&mut conn, completed_package_video).await?;
            }
            Err(err) => {
                tracing::warn!(?err, "error packaging video");
            }
        }
        Ok(())
    }
}
