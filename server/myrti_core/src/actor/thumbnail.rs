use eyre::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

use crate::{
    actor::misc::task_loop,
    catalog::{
        operation::{
            create_album_thumbnail::{self, CreateAlbumThumbnail, CreateAlbumThumbnailWithPaths},
            create_thumbnail::{
                CreateAssetThumbnail, CreateThumbnailWithPaths, ThumbnailSideEffectResult,
                ThumbnailToCreateWithPaths, apply_create_thumbnail,
                perform_side_effects_create_thumbnail,
            },
        },
        storage_key,
    },
    core::storage::Storage,
    model::{ThumbnailFormat, repository::db::DbPool},
    processing::process_control::ProcessControlReceiver,
};

use super::simple_queue_actor::{
    Actor, ActorOptions, MsgFrom, MsgTaskControl, QueuedActorHandle, TaskError, TaskId,
};

pub type ThumbnailActorHandle = QueuedActorHandle<ThumbnailTaskMsg>;
pub type MsgFromThumbnail = MsgFrom<ThumbnailTaskResult>;

#[derive(Debug, Clone)]
pub enum ThumbnailTaskMsg {
    CreateAssetThumbnail(CreateAssetThumbnail),
    CreateAlbumThumbnail(CreateAlbumThumbnail),
}

#[derive(Debug)]
pub enum ThumbnailTaskResult {
    Asset(Result<ThumbnailSideEffectResult>),
    Album(Result<CreateAlbumThumbnailWithPaths>),
}

pub fn start_thumbnail_actor(
    db_pool: DbPool,
    storage: Storage,
    did_shutdown_send: oneshot::Sender<()>,
    send_from_us: mpsc::UnboundedSender<MsgFromThumbnail>,
) -> ThumbnailActorHandle {
    let actor = ThumbnailActor { db_pool, storage };
    QueuedActorHandle::new(
        actor,
        send_from_us,
        did_shutdown_send,
        ActorOptions {
            max_tasks: 8,
            max_queue_size: 1000,
        },
        tracing::info_span!("thumbnail"),
    )
}

impl QueuedActorHandle<ThumbnailTaskMsg> {
    pub fn msg_create_asset_thumbnail(&self, msg: CreateAssetThumbnail) -> Result<()> {
        self.msg_do_task(ThumbnailTaskMsg::CreateAssetThumbnail(msg))
    }

    pub fn msg_create_album_thumbnail(&self, msg: CreateAlbumThumbnail) -> Result<()> {
        self.msg_do_task(ThumbnailTaskMsg::CreateAlbumThumbnail(msg))
    }
}

struct ThumbnailActor {
    db_pool: DbPool,
    storage: Storage,
}

impl Actor<ThumbnailTaskMsg, ThumbnailTaskResult> for ThumbnailActor {
    async fn run_task(
        &mut self,
        msg: ThumbnailTaskMsg,
        result_send: mpsc::UnboundedSender<(TaskId, Result<ThumbnailTaskResult, TaskError>)>,
        task_id: TaskId,
        mut ctl_recv: mpsc::UnboundedReceiver<MsgTaskControl>,
    ) {
        let (process_control_send, mut process_control_recv) = mpsc::channel(1);
        match msg {
            ThumbnailTaskMsg::CreateAssetThumbnail(create_thumbnail) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                tokio::task::spawn(
                    async move {
                        // ugly, rewrite this with try blocks one day hopefuly
                        let result_fut = do_asset_thumbnail_side_effects(
                            db_pool.clone(),
                            storage,
                            create_thumbnail,
                            &mut process_control_recv,
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
                        async fn apply_result(
                            db_pool: DbPool,
                            result: ThumbnailSideEffectResult,
                        ) -> Result<ThumbnailSideEffectResult> {
                            let mut conn = db_pool.get().await?;
                            if !result.failed.is_empty() {
                                for (_thumbnail, report) in &result.failed {
                                    tracing::warn!(?report, %result.file_id, "failed to create thumbnail");
                                }
                            }
                            for succeeded in &result.succeeded {
                                apply_create_thumbnail(
                                    &mut conn,
                                    result.file_id,
                                    succeeded.clone(),
                                )
                                .await?;
                            }
                            Ok(result)
                        }
                        if let Ok(result) = result {
                            let apply_result = apply_result(db_pool, result).await;
                            if let Ok(result) = apply_result {
                                result_send
                                    .send((task_id, Ok(ThumbnailTaskResult::Asset(Ok(result)))))
                                    .expect("Receiver must be alive");
                            } else {
                                // error applying to db
                                result_send
                                    .send((task_id, Ok(ThumbnailTaskResult::Asset(apply_result))))
                                    .expect("Receiver must be alive");
                            }
                        } else {
                            result_send
                                .send((task_id, Ok(ThumbnailTaskResult::Asset(result))))
                                .expect("Receiver must be alive");
                        }
                    }
                    .in_current_span(),
                );
            }
            ThumbnailTaskMsg::CreateAlbumThumbnail(create_thumbnail) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                tokio::task::spawn(
                    async move {
                        let result_fut = do_album_thumbnail_side_effects(
                            db_pool.clone(),
                            storage,
                            create_thumbnail,
                            &mut process_control_recv,
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

                        async fn apply_result(
                            db_pool: DbPool,
                            result: CreateAlbumThumbnailWithPaths,
                        ) -> Result<CreateAlbumThumbnailWithPaths> {
                            let mut conn = db_pool.get().await?;
                            create_album_thumbnail::apply_create_thumbnail(
                                &mut conn,
                                result.clone(),
                            )
                            .await?;
                            Ok(result)
                        }
                        if let Ok(result) = result {
                            let apply_result = apply_result(db_pool, result).await;
                            if let Ok(result) = apply_result {
                                result_send
                                    .send((task_id, Ok(ThumbnailTaskResult::Album(Ok(result)))))
                                    .expect("Receiver must be alive");
                            } else {
                                // error applying to db
                                result_send
                                    .send((task_id, Ok(ThumbnailTaskResult::Album(apply_result))))
                                    .expect("Receiver must be alive");
                            }
                        } else {
                            result_send
                                .send((task_id, Ok(ThumbnailTaskResult::Album(result))))
                                .expect("Receiver must be alive");
                        }
                    }
                    .in_current_span(),
                );
            }
        }
    }
}

fn resolve(op: &CreateAssetThumbnail) -> CreateThumbnailWithPaths {
    let thumbnails_to_create: Vec<ThumbnailToCreateWithPaths> = op
        .thumbnails
        .iter()
        .map(|thumb| {
            let file_keys = thumb
                .formats
                .iter()
                .copied()
                .map(|format| (format, storage_key::thumbnail(op.file_id, thumb.ty, format)))
                .collect();
            ThumbnailToCreateWithPaths {
                ty: thumb.ty,
                file_keys,
            }
        })
        .collect();
    CreateThumbnailWithPaths {
        file_id: op.file_id,
        thumbnails: thumbnails_to_create,
    }
}

#[tracing::instrument(skip(db_pool, storage), level = "trace")]
async fn do_asset_thumbnail_side_effects(
    db_pool: DbPool,
    storage: Storage,
    op: CreateAssetThumbnail,
    control_recv: &mut ProcessControlReceiver,
) -> Result<ThumbnailSideEffectResult> {
    let op_resolved = {
        let op = &op;
        let thumbnails_to_create: Vec<ThumbnailToCreateWithPaths> = op
            .thumbnails
            .iter()
            .map(|thumb| {
                let file_keys = thumb
                    .formats
                    .iter()
                    .copied()
                    .map(|format| (format, storage_key::thumbnail(op.file_id, thumb.ty, format)))
                    .collect();
                ThumbnailToCreateWithPaths {
                    ty: thumb.ty,
                    file_keys,
                }
            })
            .collect();
        CreateThumbnailWithPaths {
            file_id: op.file_id,
            thumbnails: thumbnails_to_create,
        }
    };
    perform_side_effects_create_thumbnail(
        &storage,
        db_pool.clone(),
        op_resolved.clone(),
        control_recv,
    )
    .await
}

#[tracing::instrument(skip(db_pool, storage), level = "trace")]
async fn do_album_thumbnail_side_effects(
    db_pool: DbPool,
    storage: Storage,
    op: CreateAlbumThumbnail,
    control_recv: &mut ProcessControlReceiver,
) -> Result<CreateAlbumThumbnailWithPaths> {
    let avif_key = storage_key::album_thumbnail(op.album_id, ThumbnailFormat::Avif);
    let webp_key = storage_key::album_thumbnail(op.album_id, ThumbnailFormat::Webp);
    let mut conn = db_pool.get().await?;
    let op_with_paths = CreateAlbumThumbnailWithPaths {
        album_id: op.album_id,
        size: op.size,
        file_id: op.file_id,
        avif_key,
        webp_key,
    };
    create_album_thumbnail::perform_side_effects_create_thumbnail(
        &storage,
        &mut conn,
        op_with_paths.clone(),
        control_recv,
    )
    .await?;
    Ok(op_with_paths)
}
