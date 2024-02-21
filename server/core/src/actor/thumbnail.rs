use chrono::Utc;
use deadpool_diesel;
use eyre::{Report, Result};
use tokio::sync::mpsc;
use tracing::instrument;

use crate::{
    catalog::{
        operation::create_thumbnail::{
            apply_create_thumbnail, perform_side_effects_create_thumbnail, CreateThumbnail,
            CreateThumbnailWithPaths, ThumbnailToCreateWithPaths,
        },
        storage_key,
    },
    config,
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

#[derive(Debug, Clone)]
pub enum ThumbnailMessage {
    CreateThumbnails(Vec<CreateThumbnail>),
}

#[derive(Debug)]
pub enum ThumbnailResult {
    OtherError(Report),
    ThumbnailError {
        thumbnail: CreateThumbnail,
        report: Report,
    },
}

pub struct ThumbnailActorHandle {
    pub send: mpsc::Sender<ThumbnailMessage>,
    pub recv_result: mpsc::Receiver<ThumbnailResult>,
}

impl ThumbnailActorHandle {
    pub fn new(db_pool: DbPool, storage: Storage, config: config::Config) -> Self {
        let (send, recv) = mpsc::channel(10000);
        let (send_result, recv_result) = mpsc::channel(1000);
        let actor = ThumbnailActor {
            db_pool,
            storage,
            config,
            recv,
            send_result,
        };
        tokio::spawn(run_thumbnail_actor(actor));
        Self { send, recv_result }
    }
}

struct ThumbnailActor {
    pub db_pool: DbPool,
    pub storage: Storage,
    pub config: config::Config,
    pub recv: mpsc::Receiver<ThumbnailMessage>,
    pub send_result: mpsc::Sender<ThumbnailResult>,
}

async fn run_thumbnail_actor(mut actor: ThumbnailActor) {
    while let Some(msg) = actor.recv.recv().await {
        match msg {
            ThumbnailMessage::CreateThumbnails(create_thumbnails) => {
                let res = handle_message(&mut actor, create_thumbnails).await;
                if let Err(report) = res {
                    tracing::warn!(%report, "Aborted thumbnail job");
                    let _ = actor
                        .send_result
                        .send(ThumbnailResult::OtherError(
                            report.wrap_err("error starting thumbnail job"),
                        ))
                        .await;
                }
            }
        }
    }
}

async fn handle_message(
    actor: &mut ThumbnailActor,
    create_thumbnails: Vec<CreateThumbnail>,
) -> Result<()> {
    fn resolve(op: &CreateThumbnail) -> CreateThumbnailWithPaths {
        let mut thumbnails_to_create: Vec<ThumbnailToCreateWithPaths> = Vec::default();
        for thumb in &op.thumbnails {
            let avif_key = storage_key::thumbnail(op.asset_id, thumb.ty, ThumbnailFormat::Avif);
            let webp_key = storage_key::thumbnail(op.asset_id, thumb.ty, ThumbnailFormat::Webp);
            let thumbnail_to_create = ThumbnailToCreateWithPaths {
                ty: thumb.ty,
                webp_key,
                avif_key,
            };
            thumbnails_to_create.push(thumbnail_to_create);
        }
        CreateThumbnailWithPaths {
            asset_id: op.asset_id,
            thumbnails: thumbnails_to_create,
        }
    }

    for op in create_thumbnails {
        let conn = actor.db_pool.get().await?;
        let asset_id = op.asset_id;
        let past_failed_job = interact!(conn, move |mut conn| {
            repository::failed_job::get_failed_thumbnail_job_for_asset(&mut conn, asset_id)
        })
        .await??;
        if let Some(past_failed_job) = past_failed_job {
            let asset_path = interact!(conn, move |mut conn| {
                repository::asset::get_asset_path_on_disk(&mut conn, asset_id)
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
                continue;
            }
        }
        drop(conn); // don't hold connection over long operations that don't need it

        let op_resolved = resolve(&op);
        let side_effect_results = match perform_side_effects_create_thumbnail(
            &actor.storage,
            actor.db_pool.clone(),
            op_resolved.clone(),
        )
        .await
        {
            Err(report) => {
                // same as above
                // if things fail here it's not the asset's fault, so don't remember the fail
                // in the database
                let _ = actor
                    .send_result
                    .send(ThumbnailResult::ThumbnailError {
                        thumbnail: op.clone(),
                        report,
                    })
                    .await;
                continue;
            }
            Ok(r) => r,
        };
        // if one thumbnail of op fails we discard the whole thing for now
        let mut conn = actor.db_pool.get().await?;
        if !side_effect_results.failed.is_empty() {
            for (_thumbnail, report) in side_effect_results.failed {
                let _ = actor
                    .send_result
                    .send(ThumbnailResult::ThumbnailError {
                        thumbnail: op.clone(),
                        report,
                    })
                    .await;
            }
            let saved_failed_thumbnail_res = save_failed_thumbnail(&mut conn, asset_id).await;
            if let Err(err) = saved_failed_thumbnail_res {
                tracing::warn!(%err, "failed inserting FailedThumbnailJob");
            }
        }
        let apply_result = apply_create_thumbnail(&mut conn, op_resolved).await;
        if let Err(report) = apply_result {
            let _ = actor
                .send_result
                .send(ThumbnailResult::ThumbnailError {
                    thumbnail: op.clone(),
                    report,
                })
                .await;
            continue;
        }
    }
    Ok(())
}

async fn save_failed_thumbnail(conn: &mut PooledDbConn, asset_id: AssetId) -> Result<()> {
    let asset_path = interact!(conn, move |mut conn| {
        repository::asset::get_asset_path_on_disk(&mut conn, asset_id)
    })
    .await??
    .path_on_disk();
    let file = tokio::fs::File::open(&asset_path)
        .await?
        .try_into_std()
        .expect("no operation has touched this file");
    let hash = hash_file(file).await?;
    let insert_res = interact!(conn, move |mut conn| {
        repository::failed_job::insert_failed_thumbnail_job(
            &mut conn,
            &FailedThumbnailJob {
                asset_id,
                file_hash: hash,
                date: Utc::now(),
            },
        )
    })
    .await?;
    Ok(())
}
