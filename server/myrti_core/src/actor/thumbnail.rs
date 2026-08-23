use eyre::{Result, eyre};
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

use crate::{
    actor::misc::task_loop,
    catalog::{
        operation::{
            create_album_thumbnail::{self, CreateAlbumThumbnail, CreateAlbumThumbnailWithPaths},
            create_thumbnail::{
                CreateAssetThumbhash, CreateAssetThumbnail, CreateThumbnailWithPaths,
                ThumbnailSideEffectResult, ThumbnailToCreateWithPaths, apply_create_thumbnail,
            },
        },
        rules, storage_key,
    },
    core::storage::{Storage, StorageProvider},
    interact,
    model::{
        AssetThumbnail, AssetThumbnailId, AssetType, FileId, ThumbnailFormat, ThumbnailType,
        repository::{
            self,
            db::{DbPool, PooledDbConn},
        },
    },
    processing::process_control::ProcessControlReceiver,
};

use super::simple_queue_actor::{
    Actor, ActorOptions, MsgFrom, MsgTaskControl, QueuedActorHandle, TaskError, TaskId,
};

pub type ThumbnailActorHandle = QueuedActorHandle<ThumbnailTaskMsg>;
pub type MsgFromThumbnail = MsgFrom<ThumbnailTaskResult>;

#[derive(Debug)]
pub enum ThumbnailTaskMsg {
    CreateAssetThumbnail(CreateAssetThumbnail),
    CreateAssetThumbhash(CreateAssetThumbhash),
    CreateAlbumThumbnail(CreateAlbumThumbnail),
}

#[derive(Debug)]
pub enum ThumbnailTaskResult {
    Thumbhash(FileId, Result<()>),
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
    pub fn msg_create_thumbhash(&self, msg: CreateAssetThumbhash) -> Result<()> {
        self.msg_do_task(ThumbnailTaskMsg::CreateAssetThumbhash(msg))
    }
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
            ThumbnailTaskMsg::CreateAssetThumbhash(CreateAssetThumbhash { file_id }) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();

                tokio::task::spawn(
                    async move {
                        let result = generate_thumbhash(db_pool, storage, file_id).await;
                        result_send
                            .send((task_id, Ok(ThumbnailTaskResult::Thumbhash(file_id, result))))
                            .unwrap();
                    }
                    .in_current_span(),
                );
            }
            ThumbnailTaskMsg::CreateAssetThumbnail(create_thumbnail) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                tokio::task::spawn(
                    async move {
                        let mut conn = db_pool.get().await.expect("todo");
                        // ugly, rewrite this with try blocks one day hopefuly
                        let result_fut = generate_asset_thumbnail(
                            &mut conn,
                            &storage,
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
                        result_send
                            .send((task_id, Ok(ThumbnailTaskResult::Asset(result))))
                            .expect("Receiver must be alive");
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

async fn generate_asset_thumbnail(
    conn: &mut PooledDbConn,
    storage: &Storage,
    op: CreateAssetThumbnail,
    control_recv: &mut ProcessControlReceiver,
) -> Result<ThumbnailSideEffectResult> {
    let (in_path, file) = interact!(conn, move |conn| {
        let in_path = repository::asset::get_asset_path_on_disk(conn, op.file_id)?.path_on_disk();
        let file = repository::asset::get_asset_file(conn, op.file_id)?;
        Ok::<_, eyre::Report>((in_path, file))
    })
    .await??;

    let mut result = ThumbnailSideEffectResult {
        file_id: op.file_id,
        succeeded: Vec::default(),
        failed: Vec::default(),
    };
    for thumb in op.thumbnails {
        let file_keys = thumb
            .formats
            .iter()
            .copied()
            .map(|format| (format, storage_key::thumbnail(op.file_id, thumb.ty, format)))
            .collect();
        let thumb_with_path = ThumbnailToCreateWithPaths {
            ty: thumb.ty,
            file_keys,
        };
        match crate::catalog::operation::create_thumbnail::create_thumbnail(
            in_path.clone(),
            &file,
            &thumb_with_path,
            storage,
            control_recv,
        )
        .await
        {
            Ok(result) => {
                for format in thumb.formats.iter().copied() {
                    interact!(conn, move |conn| {
                        repository::asset::insert_asset_thumbnail(
                            conn,
                            AssetThumbnail {
                                id: AssetThumbnailId(0),
                                file_id: op.file_id,
                                ty: thumb.ty,
                                size: result.actual_size,
                                format,
                            },
                        )
                    })
                    .await??;
                }
            }
            Err(err) => {
                result.failed.push((thumb_with_path.clone(), err));
            }
        }
    }
    Ok(result)
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

async fn generate_thumbhash(db_pool: DbPool, storage: Storage, file_id: FileId) -> Result<()> {
    let conn = db_pool.get().await?;
    let file = interact!(conn, move |conn| {
        repository::asset::get_asset_file(conn, file_id)
    })
    .await??;
    let input_path = match file.ty {
        AssetType::Image => interact!(conn, move |conn| {
            repository::asset::get_asset_path_on_disk(conn, file_id)
        })
        .await??
        .path_on_disk(),
        AssetType::Video => {
            let thumbnail = interact!(conn, move |conn| {
                repository::asset::get_thumbnails_for_asset(conn, file_id)
            })
            .await??
            .into_iter()
            .find(|thumb| thumb.ty == ThumbnailType::LargeOrigAspect)
            .ok_or(eyre!(
                "can't generate thumbhash for video with no suitable thumbnail yet"
            ))?;
            let thumbnail_key = storage_key::thumbnail(file_id, thumbnail.ty, thumbnail.format);
            storage
                .local_path(&thumbnail_key)
                .await?
                .ok_or(eyre!("thumbnail must be local file"))?
        }
    };
    let thumbhash = crate::processing::image::thumbnail::generate_thumbhash(input_path).await?;
    interact!(conn, move |conn| {
        repository::asset::set_file_thumbhash(conn, file_id, &thumbhash)
    })
    .await??;
    Ok(())
}
