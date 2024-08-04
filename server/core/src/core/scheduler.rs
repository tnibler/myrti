use tokio::sync::mpsc;
use tracing::instrument;

use crate::{
    actor::{
        image_conversion::{
            ImageConversionActorHandle, ImageConversionMessage, ImageConversionResult,
        },
        indexing::{MsgFromIndexing, IndexingActorHandle},
        thumbnail::{MsgFromThumbnail, ThumbnailActorHandle},
        video_packaging::{VideoPackagingActorHandle, VideoPackagingMessage, VideoPackagingResult},
    },
    catalog::rules,
    config::Config,
    interact,
    model::{
        repository::{self, album_thumbnail, db::DbPool},
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
        let (from_indexing_send, from_indexing_recv) = mpsc::unbounded_channel();
        let indexing_actor = IndexingActorHandle::new(db_pool.clone(), config.clone(), from_indexing_send);

        let (from_thumbnail_send, from_thumbnail_recv) = mpsc::unbounded_channel();
        let thumbnail_actor =
            ThumbnailActorHandle::new(db_pool.clone(), storage.clone(), from_thumbnail_send);
        // let video_packaging_actor =
        //     VideoPackagingActorHandle::new(db_pool.clone(), storage.clone(), config.clone());
        // let image_conversion_actor =
        //     ImageConversionActorHandle::new(db_pool.clone(), storage.clone(), config.clone());

        let db_pool_copy = db_pool.clone();
        // let video_packaging_send = video_packaging_actor.send.clone();
        // let image_conversion_send = image_conversion_actor.send.clone();
        let (send, recv) = mpsc::channel(1000);
        let sched = Scheduler {
            db_pool,
            storage,
            config,
            indexing_actor: indexing_actor.clone(),
            thumbnail_actor: thumbnail_actor.clone(),
        };
        tokio::spawn(run_scheduler(sched, recv, from_indexing_recv, from_thumbnail_recv));
        tokio::spawn(on_startup(
            db_pool_copy,
            indexing_actor,
            thumbnail_actor,
        ));
        Self { send }
    }
}

async fn run_scheduler(
    sched: Scheduler,
    mut recv: mpsc::Receiver<SchedulerMessage>,
    mut indexing_recv: mpsc::UnboundedReceiver<MsgFromIndexing>,
    mut thumbnail_recv: mpsc::UnboundedReceiver<MsgFromThumbnail>,
) {
    loop {
        tokio::select! {
            Some(msg) = recv.recv() => {
                sched.handle_message(msg).await;
            }
            Some(indexing_result) = indexing_recv.recv() => {
                sched.on_indexing_result(indexing_result).await;
            }
        }
    }
}

struct Scheduler {
    pub db_pool: DbPool,
    pub storage: Storage,
    pub config: Config,
    pub indexing_actor: IndexingActorHandle,
    pub thumbnail_actor: ThumbnailActorHandle,
}

impl Scheduler {
    async fn on_indexing_result(&self, indexing_result: MsgFromIndexing) {
        match indexing_result {
            MsgFromIndexing::ActivityChange { running_tasks, queued_tasks } => {},
            MsgFromIndexing::DroppedMessage => {
            },
            MsgFromIndexing::NewAsset(asset_id) => {
                self.on_new_asset_indexed(asset_id).await;
            },
            MsgFromIndexing::IndexingError { root_dir_id, path, report } => {
                tracing::error!(?root_dir_id, ?path, ?report, "TODO unhandled indexing error");
            },
            MsgFromIndexing::FailedToStartIndexing { root_dir_id, report } => {
                tracing::error!(?root_dir_id, %report, "TODO unhandled failed to start indexing job");
            },
        }
    }

    async fn on_new_asset_indexed(&self, asset_id: AssetId) {
        let mut conn = self.db_pool.get().await.unwrap();
        let thumbnails_required = rules::required_thumbnails_for_asset(&mut conn, asset_id)
            .await
            .expect("TODO");
        if !thumbnails_required.thumbnails.is_empty() {
            let _ = self.thumbnail_actor.msg_create_asset_thumbnail(thumbnails_required);
        }
        let video_packaging_required = rules::required_video_packaging_for_asset(&mut conn, asset_id)
            .await
            .expect("TODO");
        // for vid_pack in video_packaging_required {
        //     let _ = self.video_packaging_actor.send
        //         .send(VideoPackagingMessage::PackageVideo(vid_pack))
        //     .await;
        // }

        let image_conversion_required = rules::required_image_conversion_for_asset(&mut conn, asset_id)
            .await
            .expect("TODO");
        // for img_convert in image_conversion_required {
        //     let _ = self.image_conversion_actor.send
        //         .send(ImageConversionMessage::ConvertImage(img_convert))
        //     .await;
        // }
    }

    async fn handle_message(&self, msg: SchedulerMessage) {
    match msg {
        SchedulerMessage::Timer => {}
        SchedulerMessage::UserRequest(user_request) => match user_request {
            UserRequest::ReindexAssetRoot(root_dir_id) => {
                let _ = self.indexing_actor.msg_index_asset_root(root_dir_id);
            }
        },
    }
    }
}

#[instrument(skip_all)]
async fn on_startup(
    db_pool: DbPool,
    indexing_actor: IndexingActorHandle,
    thumbnail_actor: ThumbnailActorHandle,
) {
    let mut conn = db_pool
        .get()
        .await
        .expect("TODO how do we handle errors in scheduler");

    let video_packaging_required = rules::video_packaging_due(&mut conn).await.expect("TODO");
    let video_packaging_count = video_packaging_required.len();
    let image_conversion_required = rules::image_conversion_due(&mut conn).await.expect("TODO");
    let image_conversion_count = image_conversion_required.len();
    let thumbnails_required = rules::thumbnails_to_create(&mut conn).await.expect("TODO");
    let thumbnail_count = thumbnails_required.len();
    let album_thumbnails_required = rules::album_thumbnails_to_create(&mut conn)
        .await
        .expect("TODO");
    tracing::info!(
        image_conversion = image_conversion_count,
        video_packaging = video_packaging_count,
        thumbnail = thumbnail_count,
        album_thumbnail = album_thumbnails_required.len(),
        "Collected required jobs"
    );
    // for vid_pack in video_packaging_required {
    //     let _ = video_packaging_send
    //         .send(VideoPackagingMessage::PackageVideo(vid_pack))
    //         .await;
    // }
    // for img_convert in image_conversion_required {
    //     let _ = image_conversion_send
    //         .send(ImageConversionMessage::ConvertImage(img_convert))
    //         .await;
    // }
    if !thumbnails_required.is_empty() {
        for t in thumbnails_required {
            let _ = thumbnail_actor.msg_create_asset_thumbnail(t);
        }
    }
    for album_thumb in album_thumbnails_required {
        let _ = thumbnail_actor.msg_create_album_thumbnail(album_thumb);
    }

    let asset_roots = interact!(conn, move |conn| {
        repository::asset_root_dir::get_asset_roots(conn)
    })
    .await
    .expect("TODO how do we handle errors in scheduler")
    .expect("TODO how do we handle errors in scheduler");
    for asset_root in asset_roots {
        let _ = indexing_actor.msg_index_asset_root(asset_root.id);
    }
}

