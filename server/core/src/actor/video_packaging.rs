use eyre::{Report, Result};
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

use crate::{
    actor::{misc::task_loop, simple_queue_actor::TaskError},
    catalog::operation::package_video::{
        apply_package_video, perform_side_effects_package_video, CompletedPackageVideo,
        PackageVideo,
    },
    config,
    core::storage::Storage,
    model::repository::db::DbPool,
};

use super::simple_queue_actor::{
    Actor, ActorOptions, MsgFrom, MsgTaskControl, QueuedActorHandle, TaskId,
};

pub type VideoPackagingActorHandle = QueuedActorHandle<VideoPackagingTaskMsg>;
pub type MsgFromVideoPackaging = MsgFrom<VideoPackagingTaskResult>;

#[derive(Debug, Clone)]
pub enum VideoPackagingTaskMsg {
    PackageVideo(PackageVideo),
}

#[derive(Debug)]
pub enum VideoPackagingTaskResult {
    PackagingComplete(PackageVideo),
    PackagingError {
        package_video: PackageVideo,
        report: Report,
    },
}

pub fn start_video_packaging_actor(
    db_pool: DbPool,
    storage: Storage,
    config: config::Config,
    did_shutdown_send: oneshot::Sender<()>,
    send_from_us: mpsc::UnboundedSender<MsgFromVideoPackaging>,
) -> QueuedActorHandle<VideoPackagingTaskMsg> {
    let actor = VideoPackagingActor {
        db_pool,
        storage,
        config,
    };
    QueuedActorHandle::new(
        actor,
        send_from_us,
        did_shutdown_send,
        ActorOptions {
            max_tasks: 1,
            max_queue_size: 100,
        },
        tracing::info_span!("video_packaging"),
    )
}

impl QueuedActorHandle<VideoPackagingTaskMsg> {
    pub fn msg_package_video(&self, msg: PackageVideo) -> Result<()> {
        self.msg_do_task(VideoPackagingTaskMsg::PackageVideo(msg))
    }
}

struct VideoPackagingActor {
    db_pool: DbPool,
    storage: Storage,
    config: config::Config,
}

impl Actor<VideoPackagingTaskMsg, VideoPackagingTaskResult> for VideoPackagingActor {
    async fn run_task(
        &mut self,
        msg: VideoPackagingTaskMsg,
        result_send: mpsc::UnboundedSender<(TaskId, Result<VideoPackagingTaskResult, TaskError>)>,
        task_id: TaskId,
        mut ctl_recv: mpsc::UnboundedReceiver<MsgTaskControl>,
    ) {
        match msg {
            VideoPackagingTaskMsg::PackageVideo(package_video) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                let bin_paths = self.config.bin_paths.clone();
                async fn apply_result(
                    db_pool: DbPool,
                    result: CompletedPackageVideo,
                ) -> Result<()> {
                    let mut conn = db_pool.get().await?;
                    apply_package_video(&mut conn, result.clone()).await
                }
                tokio::task::spawn(
                    async move {
                        let (process_control_send, process_control_recv) =
                            tokio::sync::mpsc::channel(1);
                        let result_fut = perform_side_effects_package_video(
                            &db_pool,
                            &storage,
                            &package_video,
                            bin_paths.as_ref(),
                            process_control_recv,
                        );
                        let task_result =
                            task_loop(result_fut, &mut ctl_recv, process_control_send).await;
                        let result = match task_result {
                            Ok(r) => r,
                            Err(err) => {
                                result_send
                                    .send((task_id, Err(err)))
                                    .expect("Receiver must be alive");
                                return;
                            }
                        };
                        match result {
                            Ok(result) => {
                                let apply_result = apply_result(db_pool, result).await;
                                match apply_result {
                                    Ok(()) => {
                                        result_send
                                            .send((
                                                task_id,
                                                Ok(VideoPackagingTaskResult::PackagingComplete(
                                                    package_video,
                                                )),
                                            ))
                                            .expect("Receiver must be alive");
                                    }
                                    Err(report) => {
                                        result_send
                                            .send((
                                                task_id,
                                                Ok(VideoPackagingTaskResult::PackagingError {
                                                    package_video,
                                                    report,
                                                }),
                                            ))
                                            .expect("Receiver must be alive");
                                    }
                                }
                            }
                            Err(report) => {
                                result_send
                                    .send((
                                        task_id,
                                        Ok(VideoPackagingTaskResult::PackagingError {
                                            package_video,
                                            report,
                                        }),
                                    ))
                                    .expect("Receiver must be alive");
                            }
                        }
                    }
                    .in_current_span(),
                );
            }
        }
    }
}
