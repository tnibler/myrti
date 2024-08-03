use chrono::Utc;
use deadpool_diesel;
use eyre::Result;
use tokio::sync::mpsc;
use tracing::Instrument;

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

use super::simple_queue_actor::{
    Actor, ActorOptions, MsgFrom, MsgTaskControl, QueuedActorHandle, TaskId,
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
    send_from_us: mpsc::UnboundedSender<MsgFromThumbnail>,
) -> ThumbnailActorHandle {
    let actor = ThumbnailActor { db_pool, storage };
    QueuedActorHandle::new(
        actor,
        send_from_us,
        ActorOptions {
            max_tasks: 8,
            max_queue_size: 1000,
        },
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
    #[tracing::instrument(skip(self))]
    async fn run_task(
        &mut self,
        msg: ThumbnailTaskMsg,
        result_send: mpsc::UnboundedSender<(TaskId, ThumbnailTaskResult)>,
        task_id: TaskId,
        _ctl_recv: mpsc::UnboundedReceiver<MsgTaskControl>,
    ) {
        match msg {
            ThumbnailTaskMsg::CreateAssetThumbnail(create_thumbnail) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                tokio::task::spawn(async move {
                    // ugly, rewrite this with try blocks one day hopefuly
                    let result =
                        do_asset_thumbnail_side_effects(db_pool.clone(), storage, create_thumbnail)
                            .await;
                    async fn apply_result(
                        db_pool: DbPool,
                        result: ThumbnailSideEffectResult,
                    ) -> Result<ThumbnailSideEffectResult> {
                        let mut conn = db_pool.get().await?;
                        if !result.failed.is_empty() {
                            for (_thumbnail, report) in &result.failed {
                                tracing::warn!(?report, %result.asset_id, "failed to create thumbnail");
                            }
                            save_failed_thumbnail(&mut conn, result.asset_id).await?;
                        }
                        for succeeded in &result.succeeded {
                            apply_create_thumbnail(&mut conn, result.asset_id, succeeded.clone())
                                .await?;
                        }
                        Ok(result)
                    }
                    if let Ok(result) = result {
                        let apply_result = apply_result(db_pool, result).await;
                        if let Ok(result) = apply_result {
                            result_send
                                .send((task_id, ThumbnailTaskResult::Asset(Ok(result))))
                                .expect("Receiver must be alive");
                        } else {
                            // error applying to db
                            result_send
                                .send((task_id, ThumbnailTaskResult::Asset(apply_result)))
                                .expect("Receiver must be alive");
                        }
                    } else {
                        result_send
                            .send((task_id, ThumbnailTaskResult::Asset(result)))
                            .expect("Receiver must be alive");
                    }
                }.in_current_span());
            }
            ThumbnailTaskMsg::CreateAlbumThumbnail(create_thumbnail) => {
                let db_pool = self.db_pool.clone();
                let storage = self.storage.clone();
                tokio::task::spawn(
                    async move {
                        let result = do_album_thumbnail_side_effects(
                            db_pool.clone(),
                            storage,
                            create_thumbnail,
                        )
                        .await;

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
                                    .send((task_id, ThumbnailTaskResult::Album(Ok(result))))
                                    .expect("Receiver must be alive");
                            } else {
                                // error applying to db
                                result_send
                                    .send((task_id, ThumbnailTaskResult::Album(apply_result)))
                                    .expect("Receiver must be alive");
                            }
                        } else {
                            result_send
                                .send((task_id, ThumbnailTaskResult::Album(result)))
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
    let mut thumbnails_to_create: Vec<ThumbnailToCreateWithPaths> = Vec::default();
    for thumb in &op.thumbnails {
        let avif_key = storage_key::thumbnail(op.asset_id, thumb.ty, ThumbnailFormat::Avif);
        let webp_key = storage_key::thumbnail(op.asset_id, thumb.ty, ThumbnailFormat::Webp);
        let thumbnail_to_create = ThumbnailToCreateWithPaths {
            ty: thumb.ty,
            file_keys: vec![
                (ThumbnailFormat::Avif, avif_key),
                (ThumbnailFormat::Webp, webp_key),
            ],
        };
        thumbnails_to_create.push(thumbnail_to_create);
    }
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
