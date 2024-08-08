use eyre::Result;
use futures::{stream::FuturesUnordered, StreamExt, TryStreamExt};
use strum::EnumCount;
use tokio::sync::{mpsc, oneshot};
use tracing::instrument;

use crate::{
    actor::{
        image_conversion::{
            start_image_conversion_actor, ImageConversionActorHandle, MsgFromImageConversion,
        },
        indexing::{IndexingActorHandle, MsgFromIndexing},
        thumbnail::{start_thumbnail_actor, MsgFromThumbnail, ThumbnailActorHandle},
        video_packaging::{
            start_video_packaging_actor, MsgFromVideoPackaging, VideoPackagingActorHandle,
        },
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
    Startup,
    Timer,
    UserRequest(UserRequest),
    PauseAllProcessing,
    ResumeAllProcessing,
    PauseVideoPackaging,
    ResumeVideoPackaging,
    Shutdown,
}

#[derive(Debug)]
pub enum UserRequest {
    ReindexAssetRoot(AssetRootDirId),
}

#[derive(Debug, Clone)]
pub struct SchedulerHandle {
    pub send: mpsc::Sender<SchedulerMessage>,
}

#[derive(Debug, Copy, Clone, strum::EnumCount)]
#[repr(usize)]
enum Actors {
    Indexing,
    Thumbnail,
    ImageConversion,
    VideoPackaging,
}

#[derive(Debug, Default)]
struct ActorState {
    has_dropped_msgs: bool,
}

struct Scheduler {
    db_pool: DbPool,
    storage: Storage,
    config: Config,

    waiting_for_shutdown: bool,
    did_shutdown_send: Option<oneshot::Sender<()>>,
    actor_did_shutdown_recvs: Option<Vec<oneshot::Receiver<()>>>,

    actor_states: [ActorState; Actors::COUNT],

    indexing_actor: IndexingActorHandle,
    thumbnail_actor: ThumbnailActorHandle,
    video_packaging_actor: VideoPackagingActorHandle,
    image_conversion_actor: ImageConversionActorHandle,
}

impl SchedulerHandle {
    pub fn new(
        db_pool: DbPool,
        storage: Storage,
        config: Config,
        did_shutdown_send: oneshot::Sender<()>,
    ) -> Self {
        // TODO: indexign shutdown
        let (indexing_did_shutdown_send, indexing_did_shutdown_recv) = oneshot::channel::<()>();
        let (from_indexing_send, from_indexing_recv) = mpsc::unbounded_channel();
        let indexing_actor =
            IndexingActorHandle::new(db_pool.clone(), config.clone(), from_indexing_send);

        let (thumbnail_did_shutdown_send, thumbnail_did_shutdown_recv) = oneshot::channel();
        let (from_thumbnail_send, from_thumbnail_recv) = mpsc::unbounded_channel();
        let thumbnail_actor = start_thumbnail_actor(
            db_pool.clone(),
            storage.clone(),
            thumbnail_did_shutdown_send,
            from_thumbnail_send,
        );

        let (video_did_shutdown_send, video_did_shutdown_recv) = oneshot::channel();
        let (from_video_packaging_send, from_video_packaging_recv) = mpsc::unbounded_channel();
        let video_packaging_actor = start_video_packaging_actor(
            db_pool.clone(),
            storage.clone(),
            config.clone(),
            video_did_shutdown_send,
            from_video_packaging_send,
        );

        let (image_conversion_did_shutdown_send, image_conversion_did_shutdown_recv) =
            oneshot::channel();
        let (from_image_conversion_send, from_image_conversion_recv) = mpsc::unbounded_channel();
        let image_conversion_actor = start_image_conversion_actor(
            db_pool.clone(),
            storage.clone(),
            image_conversion_did_shutdown_send,
            from_image_conversion_send,
        );

        let (send, recv) = mpsc::channel(1000);
        let sched = Scheduler {
            db_pool,
            storage,
            config,
            waiting_for_shutdown: false,
            did_shutdown_send: Some(did_shutdown_send),
            actor_did_shutdown_recvs: Some(vec![
                thumbnail_did_shutdown_recv,
                video_did_shutdown_recv,
                image_conversion_did_shutdown_recv,
            ]),
            actor_states: Default::default(),
            indexing_actor: indexing_actor.clone(),
            thumbnail_actor: thumbnail_actor.clone(),
            video_packaging_actor: video_packaging_actor.clone(),
            image_conversion_actor: image_conversion_actor.clone(),
        };
        tokio::spawn(run_scheduler(
            sched,
            recv,
            from_indexing_recv,
            from_thumbnail_recv,
            from_video_packaging_recv,
            from_image_conversion_recv,
        ));
        Self { send }
    }
}

async fn run_scheduler(
    mut sched: Scheduler,
    mut recv: mpsc::Receiver<SchedulerMessage>,
    mut indexing_recv: mpsc::UnboundedReceiver<MsgFromIndexing>,
    mut thumbnail_recv: mpsc::UnboundedReceiver<MsgFromThumbnail>,
    mut video_packaging_recv: mpsc::UnboundedReceiver<MsgFromVideoPackaging>,
    mut image_conversion_recv: mpsc::UnboundedReceiver<MsgFromImageConversion>,
) {
    loop {
        tokio::select! {
            Some(msg) = recv.recv() => {
                sched.handle_message(msg).await;
            }
            Some(indexing_msg) = indexing_recv.recv() => {
                if let Err(err) = sched.on_indexing_msg(indexing_msg).await {
                    tracing::error!(?err, "error in scheduler");
                }
            }
            Some(thumbnail_msg) = thumbnail_recv.recv() => {
                if let Err(err) = sched.on_thumbnail_msg(thumbnail_msg).await {
                    tracing::error!(?err, "error in scheduler");
                }
            }
            Some(video_packaging_msg) = video_packaging_recv.recv() => {
                if let Err(err) = sched.on_video_packaging_msg(video_packaging_msg).await {
                    tracing::error!(?err, "error in scheduler");
                }
            }
            Some(image_conversion_msg) = image_conversion_recv.recv() => {
                if let Err(err) = sched.on_image_conversion_msg(image_conversion_msg).await {
                    tracing::error!(?err, "error in scheduler");
                }
            }
            else => {
                break;
            }
        }
    }
}

impl Scheduler {
    async fn on_indexing_msg(&mut self, msg: MsgFromIndexing) -> Result<()> {
        let actor_state = &mut self.actor_states[Actors::Indexing as usize];
        match msg {
            MsgFromIndexing::ActivityChange {
                running_tasks,
                queued_tasks,
            } => {
                let is_idle = running_tasks == 0 && queued_tasks == 0;
                if is_idle && actor_state.has_dropped_msgs {
                    actor_state.has_dropped_msgs = false;
                    // TODO: what do we do here. reindex all? for other actors we look up in db
                    // what work they can do but this one is different
                }
            }
            MsgFromIndexing::DroppedMessage => {
                actor_state.has_dropped_msgs = true;
            }
            MsgFromIndexing::NewAsset(asset_id) => {
                if let Err(err) = self.on_new_asset_indexed(asset_id).await {
                    tracing::error!(?err, "error in on_new_asset_indexed");
                }
            }
            MsgFromIndexing::IndexingError {
                root_dir_id,
                path,
                report,
            } => {
                tracing::error!(
                    ?root_dir_id,
                    ?path,
                    ?report,
                    "TODO unhandled indexing error"
                );
            }
            MsgFromIndexing::FailedToStartIndexing {
                root_dir_id,
                report,
            } => {
                tracing::error!(?root_dir_id, %report, "TODO unhandled failed to start indexing job");
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn on_new_asset_indexed(&self, asset_id: AssetId) -> Result<()> {
        let mut conn = self.db_pool.get().await.unwrap();
        let thumbnails_required = rules::required_thumbnails_for_asset(&mut conn, asset_id).await?;
        if !thumbnails_required.thumbnails.is_empty() {
            self.thumbnail_actor
                .msg_create_asset_thumbnail(thumbnails_required)
                .expect("receiver must be alive");
        }
        let video_packaging_required =
            rules::required_video_packaging_for_asset(&mut conn, asset_id).await?;
        for vid_pack in video_packaging_required {
            self.video_packaging_actor
                .msg_package_video(vid_pack)
                .expect("receiver must be alive");
        }

        let image_conversion_required =
            rules::required_image_conversion_for_asset(&mut conn, asset_id).await?;
        for img_convert in image_conversion_required {
            self.image_conversion_actor
                .msg_convert_image(img_convert)
                .expect("receiver must be alive");
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn on_thumbnail_msg(&mut self, msg: MsgFromThumbnail) -> Result<()> {
        let actor_state = &mut self.actor_states[Actors::Thumbnail as usize];
        match msg {
            MsgFromThumbnail::ActivityChange {
                is_running,
                active_tasks,
                queued_tasks,
            } => {
                let is_idle = is_running && active_tasks == 0 && queued_tasks == 0;
                let found_new_work = if is_idle && actor_state.has_dropped_msgs {
                    actor_state.has_dropped_msgs = false;
                    let mut conn = self.db_pool.get().await?;
                    let thumbnails_required = rules::thumbnails_to_create(&mut conn).await?;
                    let any_work = !thumbnails_required.is_empty();
                    for t in thumbnails_required {
                        self.thumbnail_actor
                            .msg_create_asset_thumbnail(t)
                            .expect("receiver must be alive");
                    }
                    any_work
                } else {
                    false
                };
                if is_idle && !found_new_work {
                    tracing::info!("Thumbnail actor idle");
                }
            }
            MsgFromThumbnail::DroppedMessage => {
                actor_state.has_dropped_msgs = true;
            }
            MsgFromThumbnail::TaskResult(result) => {
                tracing::debug!(?result);
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn on_video_packaging_msg(&mut self, msg: MsgFromVideoPackaging) -> Result<()> {
        let actor_state = &mut self.actor_states[Actors::VideoPackaging as usize];
        match msg {
            MsgFromVideoPackaging::ActivityChange {
                is_running,
                active_tasks,
                queued_tasks,
            } => {
                let is_idle = is_running && active_tasks == 0 && queued_tasks == 0;
                let found_new_work = if is_idle && actor_state.has_dropped_msgs {
                    actor_state.has_dropped_msgs = false;
                    let mut conn = self.db_pool.get().await?;
                    let video_packaging_required = rules::video_packaging_due(&mut conn).await?;
                    let any_work = !video_packaging_required.is_empty();
                    for v in video_packaging_required {
                        self.video_packaging_actor
                            .msg_package_video(v)
                            .expect("receiver must be alive");
                    }
                    any_work
                } else {
                    false
                };
                if is_idle && !found_new_work {
                    tracing::info!("VideoPackaging actor idle");
                }
            }
            MsgFromVideoPackaging::DroppedMessage => {
                actor_state.has_dropped_msgs = true;
            }
            MsgFromVideoPackaging::TaskResult(result) => {
                tracing::debug!(?result);
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn on_image_conversion_msg(&mut self, msg: MsgFromImageConversion) -> Result<()> {
        let actor_state = &mut self.actor_states[Actors::ImageConversion as usize];
        match msg {
            MsgFromImageConversion::ActivityChange {
                is_running,
                active_tasks,
                queued_tasks,
            } => {
                let is_idle = is_running && active_tasks == 0 && queued_tasks == 0;
                let found_new_work = if is_idle && actor_state.has_dropped_msgs {
                    actor_state.has_dropped_msgs = false;
                    let mut conn = self.db_pool.get().await?;
                    let image_conversion_required = rules::image_conversion_due(&mut conn).await?;
                    let any_work = !image_conversion_required.is_empty();
                    for i in image_conversion_required {
                        self.image_conversion_actor
                            .msg_convert_image(i)
                            .expect("receiver must be alive");
                    }
                    any_work
                } else {
                    false
                };
                if is_idle && !found_new_work {
                    tracing::info!("ImageConversion actor idle");
                }
            }
            MsgFromImageConversion::DroppedMessage => {
                actor_state.has_dropped_msgs = true;
            }
            MsgFromImageConversion::TaskResult(result) => {
                tracing::debug!(?result);
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn handle_message(&mut self, msg: SchedulerMessage) {
        match msg {
            SchedulerMessage::Timer => {}
            SchedulerMessage::UserRequest(user_request) => match user_request {
                UserRequest::ReindexAssetRoot(root_dir_id) => {
                    let _ = self.indexing_actor.msg_index_asset_root(root_dir_id);
                }
            },
            SchedulerMessage::PauseAllProcessing => {
                self.thumbnail_actor
                    .msg_pause_all()
                    .expect("receiver must be alive");
                self.video_packaging_actor
                    .msg_pause_all()
                    .expect("receiver must be alive");
                self.image_conversion_actor
                    .msg_pause_all()
                    .expect("receiver must be alive");
            }
            SchedulerMessage::ResumeAllProcessing => {
                self.thumbnail_actor
                    .msg_resume_all()
                    .expect("receiver must be alive");
                self.video_packaging_actor
                    .msg_resume_all()
                    .expect("receiver must be alive");
                self.image_conversion_actor
                    .msg_resume_all()
                    .expect("receiver must be alive");
            }
            SchedulerMessage::PauseVideoPackaging => {
                self.video_packaging_actor
                    .msg_pause_all()
                    .expect("receiver must be alive");
            }
            SchedulerMessage::ResumeVideoPackaging => {
                self.video_packaging_actor
                    .msg_resume_all()
                    .expect("receiver must be alive");
            }
            SchedulerMessage::Shutdown => {
                if !self.waiting_for_shutdown {
                    self.waiting_for_shutdown = true;
                    self.video_packaging_actor
                        .msg_shutdown()
                        .expect("receiver must be alive");
                    self.thumbnail_actor
                        .msg_shutdown()
                        .expect("receiver must be alive");
                    self.image_conversion_actor
                        .msg_shutdown()
                        .expect("receiver must be alive");
                    let did_shutdown_recvs = self
                        .actor_did_shutdown_recvs
                        .take()
                        .expect("must be Some before shutdown called");
                    let did_shutdown_send = self
                        .did_shutdown_send
                        .take()
                        .expect("must be Some before shutdown called");
                    tokio::task::spawn(async move {
                        // for recv in did_shutdown_recvs {
                        //     recv.await.expect("TODO senders must be alive");
                        //     tracing::info!("one task shutdown");
                        // }
                        did_shutdown_recvs
                            .into_iter()
                            .collect::<FuturesUnordered<_>>()
                            .try_collect::<Vec<()>>()
                            .await
                            .expect("TODO senders must be alive");
                        tracing::info!("all actors shutdown");
                        did_shutdown_send.send(()).expect("receiver must be alive");
                    });
                }
            }
            SchedulerMessage::Startup => {
                tokio::spawn(on_startup(
                    self.db_pool.clone(),
                    self.indexing_actor.clone(),
                    self.thumbnail_actor.clone(),
                    self.video_packaging_actor.clone(),
                    self.image_conversion_actor.clone(),
                ));
            }
        }
    }
}

#[instrument(skip_all)]
async fn on_startup(
    db_pool: DbPool,
    indexing_actor: IndexingActorHandle,
    thumbnail_actor: ThumbnailActorHandle,
    video_packaging_actor: VideoPackagingActorHandle,
    image_conversion_actor: ImageConversionActorHandle,
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
    for vid_pack in video_packaging_required {
        let _ = video_packaging_actor.msg_package_video(vid_pack);
    }
    for img_convert in image_conversion_required {
        let _ = image_conversion_actor.msg_convert_image(img_convert);
    }
    for t in thumbnails_required {
        let _ = thumbnail_actor.msg_create_asset_thumbnail(t);
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
