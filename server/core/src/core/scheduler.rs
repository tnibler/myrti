use tokio::sync::mpsc;
use tracing::instrument;

use crate::{
    actor::{
        image_conversion::{ImageConversionActorHandle, ImageConversionMessage},
        indexing::{IndexingActorHandle, IndexingMessage, IndexingResult},
        thumbnail::{ThumbnailActorHandle, ThumbnailMessage},
        video_packaging::{VideoPackagingActorHandle, VideoPackagingMessage},
    },
    catalog::rules,
    config::Config,
    interact,
    model::{
        repository::{self, db::DbPool},
        AssetId, AssetRootDirId,
    },
};

use super::storage::Storage;

#[derive(Debug)]
pub enum SchedulerMessage {
    Timer,
    UserRequest(UserRequest),
}

#[derive(Debug)]
pub enum UserRequest {
    ReindexAssetRoot(AssetRootDirId),
}

#[derive(Debug, Clone)]
pub struct SchedulerHandle {
    pub send: mpsc::Sender<SchedulerMessage>,
}

impl SchedulerHandle {
    pub fn new(db_pool: DbPool, storage: Storage, config: Config) -> Self {
        let indexing_actor = IndexingActorHandle::new(db_pool.clone(), config.clone());
        let thumbnail_actor =
            ThumbnailActorHandle::new(db_pool.clone(), storage.clone(), config.clone());
        let video_packaging_actor =
            VideoPackagingActorHandle::new(db_pool.clone(), storage.clone(), config.clone());
        let image_conversion_actor =
            ImageConversionActorHandle::new(db_pool.clone(), storage.clone(), config.clone());

        let db_pool_copy = db_pool.clone();
        let indexing_send = indexing_actor.send.clone();
        let (send, recv) = mpsc::channel(1000);
        let sched = Scheduler {
            db_pool,
            storage,
            config,
            recv,
            indexing_actor,
            thumbnail_actor,
            video_packaging_actor,
            image_conversion_actor,
        };
        tokio::spawn(run_scheduler(sched));
        tokio::spawn(on_startup(db_pool_copy, indexing_send));
        Self { send }
    }
}

struct Scheduler {
    pub db_pool: DbPool,
    pub storage: Storage,
    pub config: Config,
    pub recv: mpsc::Receiver<SchedulerMessage>,
    pub indexing_actor: IndexingActorHandle,
    pub thumbnail_actor: ThumbnailActorHandle,
    pub video_packaging_actor: VideoPackagingActorHandle,
    pub image_conversion_actor: ImageConversionActorHandle,
}

async fn run_scheduler(sched: Scheduler) {
    let Scheduler {
        db_pool,
        storage,
        config,
        mut recv,
        mut indexing_actor,
        thumbnail_actor,
        video_packaging_actor,
        image_conversion_actor,
    } = sched;
    loop {
        tokio::select! {
            Some(msg) = recv.recv() => {
                handle_msg(msg, &indexing_actor.send).await;
            }
            Some(indexing_result) = indexing_actor.recv_result.recv() => {
                match indexing_result {
                    IndexingResult::NewAsset(asset_id) => {
                        on_new_asset_indexed(asset_id,
                            db_pool.clone(),
                            thumbnail_actor.send.clone(),
                            video_packaging_actor.send.clone(),
                            image_conversion_actor.send.clone(),
                        ).await;
                    },
                    IndexingResult::IndexingError { root_dir_id, path, report } => {
                        tracing::error!(?root_dir_id, ?path, ?report, "TODO unhandled indexing error");
                    },
                    IndexingResult::FailedToStartIndexing { root_dir_id, report } => {
                        tracing::error!(?root_dir_id, %report, "TODO unhandled failed to start indexing job");
                    },
                }
            }
        }
    }
}

#[instrument(skip_all, level = "debug")]
async fn on_startup(db_pool: DbPool, indexing_send: mpsc::Sender<IndexingMessage>) {
    let conn = db_pool
        .get()
        .await
        .expect("TODO how do we handle errors in scheduler");
    let asset_roots = interact!(conn, move |mut conn| {
        repository::asset_root_dir::get_asset_roots(&mut conn)
    })
    .await
    .expect("TODO how do we handle errors in scheduler")
    .expect("TODO how do we handle errors in scheduler");
    for asset_root in asset_roots {
        let _ = indexing_send
            .send(IndexingMessage::IndexAssetRootDir {
                root_dir_id: asset_root.id,
            })
            .await;
    }
}

async fn handle_msg(msg: SchedulerMessage, indexing_send: &mpsc::Sender<IndexingMessage>) {
    match msg {
        SchedulerMessage::Timer => {}
        SchedulerMessage::UserRequest(user_request) => match user_request {
            UserRequest::ReindexAssetRoot(root_dir_id) => {
                let _ = indexing_send
                    .send(IndexingMessage::IndexAssetRootDir { root_dir_id })
                    .await;
            }
        },
    }
}

async fn on_new_asset_indexed(
    asset_id: AssetId,
    db_pool: DbPool,
    thumbnail_send: mpsc::Sender<ThumbnailMessage>,
    video_packaging_send: mpsc::Sender<VideoPackagingMessage>,
    image_conversion_send: mpsc::Sender<ImageConversionMessage>,
) {
    let mut conn = db_pool.get().await.unwrap();
    let thumbnails_required = rules::required_thumbnails_for_asset(&mut conn, asset_id)
        .await
        .expect("TODO");
    if !thumbnails_required.thumbnails.is_empty() {
        let _ = thumbnail_send
            .send(ThumbnailMessage::CreateThumbnails(vec![
                thumbnails_required,
            ]))
            .await;
    }
    let video_packaging_required = rules::required_video_packaging_for_asset(&mut conn, asset_id)
        .await
        .expect("TODO");
    for vid_pack in video_packaging_required {
        let _ = video_packaging_send
            .send(VideoPackagingMessage::PackageVideo(vid_pack))
            .await;
    }

    let image_conversion_required = rules::required_image_conversion_for_asset(&mut conn, asset_id)
        .await
        .expect("TODO");
    for img_convert in image_conversion_required {
        let _ = image_conversion_send
            .send(ImageConversionMessage::ConvertImage(img_convert))
            .await;
    }
}
