use std::collections::VecDeque;

use chrono::Utc;
use deadpool_diesel;
use eyre::{Report, Result};
use tokio::sync::mpsc;

use crate::{
    catalog::{
        operation::{
            create_album_thumbnail::{self, CreateAlbumThumbnail, CreateAlbumThumbnailWithPaths},
            create_thumbnail::{
                apply_create_thumbnail, perform_side_effects_create_thumbnail,
                CreateAssetThumbnail, CreateThumbnailWithPaths, ThumbnailSideEffectResult,
                ThumbnailToCreateWithPaths,
            },
        },
        storage_key,
    },
    core::storage::Storage,
    interact,
    model::{
        repository::{
            self,
            db::{DbPool, PooledDbConn},
        },
        AssetId, FailedThumbnailJob, ThumbnailFormat,
    },
    processing::hash::hash_file,
};

#[derive(Debug)]
pub enum MsgFromThumbnail {
    ActivityChange {
        running: usize,
        queued: usize,
    },
    DroppedMessage,
    AssetThumbnailError {
        thumbnail: CreateAssetThumbnail,
        report: Report,
    },
    AlbumThumbnailError {
        thumbnail: CreateAlbumThumbnail,
        report: Report,
    },
}

#[derive(Clone)]
pub struct ThumbnailActorHandle {
    send: mpsc::UnboundedSender<ToThumbnailMsg>,
}

#[derive(Debug, Clone)]
enum ToThumbnailMsg {
    Pause,
    Resume,
    DoTask(DoTaskMsg),
}

#[derive(Debug, Clone)]
enum DoTaskMsg {
    CreateAssetThumbnail(CreateAssetThumbnail),
    CreateAlbumThumbnail(CreateAlbumThumbnail),
}

impl ThumbnailActorHandle {
    pub fn new(
        db_pool: DbPool,
        storage: Storage,
        send_from_us: mpsc::UnboundedSender<MsgFromThumbnail>,
    ) -> Self {
        let (send, recv) = mpsc::unbounded_channel::<ToThumbnailMsg>();
        let (task_result_send, task_result_recv) = mpsc::unbounded_channel::<TaskResult>();
        let actor = ThumbnailActor {
            db_pool,
            storage,
            send_from_us,
            task_result_send,
        };
        tokio::spawn(run_thumbnail_actor(recv, task_result_recv, actor));
        Self { send }
    }

    pub fn msg_create_asset_thumbnail(&self, msg: CreateAssetThumbnail) -> Result<()> {
        self.send
            .send(ToThumbnailMsg::DoTask(DoTaskMsg::CreateAssetThumbnail(msg)))?;
        Ok(())
    }

    pub fn msg_create_album_thumbnail(&self, msg: CreateAlbumThumbnail) -> Result<()> {
        self.send
            .send(ToThumbnailMsg::DoTask(DoTaskMsg::CreateAlbumThumbnail(msg)))?;
        Ok(())
    }

    pub fn msg_pause(&self) -> Result<()> {
        self.send.send(ToThumbnailMsg::Pause)?;
        Ok(())
    }

    pub fn msg_resume(&self) -> Result<()> {
        self.send.send(ToThumbnailMsg::Resume)?;
        Ok(())
    }
}

/// Result of side effect, internal
#[derive(Debug)]
enum TaskResult {
    Asset(Result<ThumbnailSideEffectResult>),
    Album(Result<CreateAlbumThumbnailWithPaths>),
}

struct ThumbnailActor {
    db_pool: DbPool,
    storage: Storage,
    send_from_us: mpsc::UnboundedSender<MsgFromThumbnail>,
    task_result_send: mpsc::UnboundedSender<TaskResult>,
}

const MAX_TASKS: usize = 4;
const MAX_QUEUE_SIZE: usize = 10;
async fn run_thumbnail_actor(
    mut actor_recv: mpsc::UnboundedReceiver<ToThumbnailMsg>,
    mut task_result_recv: mpsc::UnboundedReceiver<TaskResult>,
    actor: ThumbnailActor,
) {
    let mut is_running = true;
    let mut running_tasks: usize = 0;
    let mut queue: VecDeque<DoTaskMsg> = Default::default();
    loop {
        tokio::select! {
            Some(msg) = actor_recv.recv() => {
                match msg {
                    ToThumbnailMsg::Pause => {
                        is_running = false;
                    }
                    ToThumbnailMsg::Resume => {
                        is_running = true;
                    }
                    ToThumbnailMsg::DoTask(task) => {
                        if is_running && running_tasks < MAX_TASKS {
                            tracing::debug!(?task, "received msg, processing immediately");
                            running_tasks += 1;
                            let _ = actor.send_from_us.send(MsgFromThumbnail::ActivityChange {
                                running: running_tasks,
                                queued: queue.len(),
                            });
                            actor.process_message(task).await;
                        } else if queue.len() < MAX_QUEUE_SIZE {
                            tracing::debug!("received msg, queuing it");
                            queue.push_back(task);
                            let _ = actor.send_from_us.send(MsgFromThumbnail::ActivityChange {
                                running: running_tasks,
                                queued: queue.len(),
                            });
                        } else {
                            let _ = actor.send_from_us.send(MsgFromThumbnail::DroppedMessage);
                            tracing::debug!("received msg, queue full, dropping");
                        }
                    }
                }
            }
            Some(task_result) = task_result_recv.recv() => {
                tracing::debug!("received task result");
                running_tasks -= 1;
                if !is_running || (queue.is_empty() && running_tasks == 0) {
                    tracing::debug!("no more messages, idle");
                } else if let Some(msg) = queue.pop_front() {
                    tracing::debug!("dequeuing message");
                    actor.process_message(msg).await;
                    running_tasks += 1;
                }
                let _ = actor.send_from_us.send(MsgFromThumbnail::ActivityChange {
                    running: running_tasks,
                    queued: queue.len(),
                });
                let handling_result = match task_result {
                    TaskResult::Asset(result) => actor.on_asset_thumbnail_result(result).await,
                    TaskResult::Album(result) => actor.on_album_thumbnail_result(result).await,
                };
                if let Err(err) = handling_result {
                    // TODO: do something
                    tracing::error!(?err, "error applying operation");
                }
            }
        }
    }
}

impl ThumbnailActor {
    #[tracing::instrument(skip(self))]
    pub async fn process_message(&self, msg: DoTaskMsg) {
        match msg {
            DoTaskMsg::CreateAssetThumbnail(create_thumbnail) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                let result_send = self.task_result_send.clone();
                tokio::task::spawn(async move {
                    let result =
                        do_asset_thumbnail_side_effects(db_pool, storage, create_thumbnail).await;
                    result_send.send(TaskResult::Asset(result)).expect("TODO this error must be handled, since the work won't be written to db which is relevant");
                });
            }
            DoTaskMsg::CreateAlbumThumbnail(create_thumbnail) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                let result_send = self.task_result_send.clone();
                tokio::task::spawn(async move {
                    let result =
                        do_album_thumbnail_side_effects(db_pool, storage, create_thumbnail).await;
                    result_send.send(TaskResult::Album(result)).expect("TODO this error must be handled, since the work won't be written to db which is relevant");
                });
            }
        }
    }

    #[tracing::instrument(skip(self, result))]
    pub async fn on_asset_thumbnail_result(
        &self,
        result: Result<ThumbnailSideEffectResult>,
    ) -> Result<()> {
        match result {
            Ok(results) => {
                let mut conn = self.db_pool.get().await?;
                if !results.failed.is_empty() {
                    for (_thumbnail, report) in results.failed {
                        tracing::warn!(?report, %results.asset_id, "failed to create thumbnail");
                        // TODO: send error somewhere
                    }
                    save_failed_thumbnail(&mut conn, results.asset_id).await?;
                }
                for succeeded in results.succeeded {
                    apply_create_thumbnail(&mut conn, results.asset_id, succeeded).await?;
                }
            }
            Err(err) => {
                // TODO: I think this error is bad and is exceptional
                tracing::warn!(%err, "exceptional error creating asset thumbnails");
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, result))]
    pub async fn on_album_thumbnail_result(
        &self,
        result: Result<CreateAlbumThumbnailWithPaths>,
    ) -> Result<()> {
        let mut conn = self.db_pool.get().await?;
        match result {
            Ok(op_with_paths) => {
                create_album_thumbnail::apply_create_thumbnail(&mut conn, op_with_paths).await?;
            }
            Err(err) => {
                tracing::warn!(%err, "exceptional error creating album");
            }
        }
        Ok(())
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
                .map(|format| {
                    (
                        format,
                        storage_key::thumbnail(op.asset_id, thumb.ty, format),
                    )
                })
                .collect();
            ThumbnailToCreateWithPaths {
                ty: thumb.ty,
                file_keys,
            }
        })
        .collect();
    CreateThumbnailWithPaths {
        asset_id: op.asset_id,
        thumbnails: thumbnails_to_create,
    }
}

#[tracing::instrument(skip(db_pool, storage))]
async fn do_asset_thumbnail_side_effects(
    db_pool: DbPool,
    storage: Storage,
    op: CreateAssetThumbnail,
) -> Result<ThumbnailSideEffectResult> {
    let conn = db_pool.get().await?;
    let asset_id = op.asset_id;
    let past_failed_job = interact!(conn, move |conn| {
        repository::failed_job::get_failed_thumbnail_job_for_asset(conn, asset_id)
    })
    .await??;
    if let Some(past_failed_job) = past_failed_job {
        let asset_path = interact!(conn, move |conn| {
            repository::asset::get_asset_path_on_disk(conn, asset_id)
        })
        .await??
        .path_on_disk();
        let file = tokio::fs::File::open(&asset_path)
            .await?
            .try_into_std()
            .expect("no operation has touched this file");
        let current_hash = hash_file(file).await?;
        if current_hash == past_failed_job.file_hash {
            tracing::debug!(
                asset_id = ?asset_id,
                "skipping thumbnail that failed in the past"
            );
            // FIXME: we're not actually skipping anything here. return optional result and add
            // flag to force retry (force if user asks to create thumbnail and on startup, don't
            // force otherwise)
        }
    }
    drop(conn); // don't hold connection over long operations that don't need it

    let op_resolved = resolve(&op);
    perform_side_effects_create_thumbnail(&storage, db_pool.clone(), op_resolved.clone()).await
}

#[tracing::instrument(skip(db_pool, storage))]
async fn do_album_thumbnail_side_effects(
    db_pool: DbPool,
    storage: Storage,
    op: CreateAlbumThumbnail,
) -> Result<CreateAlbumThumbnailWithPaths> {
    let avif_key = storage_key::album_thumbnail(op.album_id, ThumbnailFormat::Avif);
    let webp_key = storage_key::album_thumbnail(op.album_id, ThumbnailFormat::Webp);
    let mut conn = db_pool.get().await?;
    let op_with_paths = CreateAlbumThumbnailWithPaths {
        album_id: op.album_id,
        size: op.size,
        asset_id: op.asset_id,
        avif_key,
        webp_key,
    };
    create_album_thumbnail::perform_side_effects_create_thumbnail(
        &storage,
        &mut conn,
        op_with_paths.clone(),
    )
    .await?;
    Ok(op_with_paths)
}

#[tracing::instrument(skip(conn))]
async fn save_failed_thumbnail(conn: &mut PooledDbConn, asset_id: AssetId) -> Result<()> {
    let asset_path = interact!(conn, move |conn| {
        repository::asset::get_asset_path_on_disk(conn, asset_id)
    })
    .await??
    .path_on_disk();
    let file = tokio::fs::File::open(&asset_path)
        .await?
        .try_into_std()
        .expect("no operation has touched this file");
    let hash = hash_file(file).await?;
    interact!(conn, move |conn| {
        repository::failed_job::insert_failed_thumbnail_job(
            conn,
            &FailedThumbnailJob {
                asset_id,
                file_hash: hash,
                date: Utc::now(),
            },
        )
    })
    .await??;
    Ok(())
}
