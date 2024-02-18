use eyre::{Report, Result};
use tokio::sync::mpsc;

use crate::{
    catalog::operation::package_video::{
        apply_package_video, perform_side_effects_package_video, PackageVideo,
    },
    config,
    core::storage::Storage,
    model::repository::db::DbPool,
};

#[derive(Debug, Clone)]
pub enum VideoPackagingMessage {
    PackageVideo(PackageVideo),
}

#[derive(Debug)]
pub enum VideoPackagingResult {
    PackagingComplete(PackageVideo),
    PackagingError {
        package_video: PackageVideo,
        report: Report,
    },
}

pub struct VideoPackagingActorHandle {
    pub send: mpsc::Sender<VideoPackagingMessage>,
    pub recv_result: mpsc::Receiver<VideoPackagingResult>,
}

impl VideoPackagingActorHandle {
    pub fn new(db_pool: DbPool, storage: Storage, config: config::Config) -> Self {
        let (send, recv) = mpsc::channel(10000);
        let (send_result, recv_result) = mpsc::channel(1000);
        let actor = VideoPackagingActor {
            db_pool,
            storage,
            config,
            recv,
            send_result,
        };
        tokio::spawn(run_video_packaging_actor(actor));
        Self { send, recv_result }
    }
}

struct VideoPackagingActor {
    pub db_pool: DbPool,
    pub storage: Storage,
    pub config: config::Config,
    pub recv: mpsc::Receiver<VideoPackagingMessage>,
    pub send_result: mpsc::Sender<VideoPackagingResult>,
}

async fn run_video_packaging_actor(mut actor: VideoPackagingActor) {
    while let Some(msg) = actor.recv.recv().await {
        match msg {
            VideoPackagingMessage::PackageVideo(package_video) => {
                let res = handle_message(&mut actor, &package_video).await;
                if let Err(report) = res {
                    let _ = actor
                        .send_result
                        .send(VideoPackagingResult::PackagingError {
                            package_video,
                            report: report.wrap_err("error running video packaging task"),
                        })
                        .await;
                }
            }
        };
    }
}

async fn handle_message(
    actor: &mut VideoPackagingActor,
    package_video: &PackageVideo,
) -> Result<()> {
    let completed_package_video = perform_side_effects_package_video(
        actor.db_pool.clone(),
        &actor.storage,
        &package_video,
        actor.config.bin_paths.as_ref(),
    )
    .await?;
    let mut conn = actor.db_pool.get().await?;
    apply_package_video(&mut conn, completed_package_video).await?;
    Ok(())
}
