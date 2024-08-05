use std::collections::HashMap;

use eyre::{Report, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{
    catalog::operation::package_video::{
        apply_package_video, perform_side_effects_package_video, CompletedPackageVideo,
        PackageVideo,
    },
    config,
    core::storage::Storage,
    model::repository::db::DbPool,
    processing::process_control::ProcessControl,
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
        result_send: mpsc::UnboundedSender<(TaskId, VideoPackagingTaskResult)>,
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
                let (process_control_send, process_control_recv) = tokio::sync::mpsc::channel(1);
                let cancel_pipe = CancellationToken::new();
                let cancel_pipe2 = cancel_pipe.clone();
                tokio::task::spawn(
                    async move {
                        loop {
                            tokio::select! {
                                _ = cancel_pipe2.cancelled() => {
                                    break;
                                }
                                Some(msg) = ctl_recv.recv() => {
                                    let process_control = match msg {
                                        MsgTaskControl::Pause => ProcessControl::Suspend,
                                        MsgTaskControl::Resume => ProcessControl::Resume,
                                        MsgTaskControl::Cancel => ProcessControl::Quit,
                                    };
                                    process_control_send.send(process_control).await.expect("TODO");
                                }
                            }
                        }
                    }
                    .in_current_span(),
                );
                tokio::task::spawn(
                    async move {
                        let result = perform_side_effects_package_video(
                            &db_pool,
                            &storage,
                            &package_video,
                            bin_paths.as_ref(),
                            process_control_recv,
                        )
                        .await;
                        cancel_pipe.cancel();
                        match result {
                            Ok(result) => {
                                let apply_result = apply_result(db_pool, result).await;
                                match apply_result {
                                    Ok(()) => {
                                        result_send
                                            .send((
                                                task_id,
                                                VideoPackagingTaskResult::PackagingComplete(
                                                    package_video,
                                                ),
                                            ))
                                            .expect("Receiver must be alive");
                                    }
                                    Err(report) => {
                                        result_send
                                            .send((
                                                task_id,
                                                VideoPackagingTaskResult::PackagingError {
                                                    package_video,
                                                    report,
                                                },
                                            ))
                                            .expect("Receiver must be alive");
                                    }
                                }
                            }
                            Err(report) => {
                                result_send
                                    .send((
                                        task_id,
                                        VideoPackagingTaskResult::PackagingError {
                                            package_video,
                                            report,
                                        },
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
