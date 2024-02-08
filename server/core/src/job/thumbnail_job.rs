use std::collections::HashSet;

use async_trait::async_trait;
use chrono::Utc;
use eyre::{Context, Report, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, Instrument};

use crate::{
    catalog::{
        operation::create_thumbnail::{
            apply_create_thumbnail, perform_side_effects_create_thumbnail, CreateThumbnail,
            CreateThumbnailWithPaths, ThumbnailToCreateWithPaths,
        },
        storage_key,
    },
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        storage::Storage,
    },
    interact,
    model::{
        repository::{self, db::DbPool},
        AssetId, FailedThumbnailJob, ThumbnailFormat,
    },
    processing::hash::hash_file,
};

pub struct ThumbnailJob {
    params: ThumbnailJobParams,
    pool: DbPool,
    storage: Storage,
}

#[derive(Debug, Clone)]
pub struct ThumbnailJobParams {
    pub thumbnails: Vec<CreateThumbnail>,
}

#[derive(Debug)]
pub struct FailedThumbnail {
    pub thumbnail: CreateThumbnail,
    pub err: Report,
}

#[derive(Debug)]
pub struct ThumbnailJobResult {
    pub failed: Vec<FailedThumbnail>,
}

impl ThumbnailJob {
    pub fn new(params: ThumbnailJobParams, pool: DbPool, storage: Storage) -> ThumbnailJob {
        ThumbnailJob {
            params,
            pool,
            storage,
        }
    }

    #[instrument(name = "ThumbnailJob", skip(self, status_tx))]
    async fn run(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> Result<ThumbnailJobResult> {
        async fn resolve(op: &CreateThumbnail) -> CreateThumbnailWithPaths {
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

        // TODO send progress updates
        status_tx
            .send(JobProgress {
                percent: None,
                description: "".to_string(),
            })
            .await
            .unwrap();
        let conn = self.pool.get().in_current_span().await?;
        let mut failed: Vec<FailedThumbnail> = Vec::default();
        for op in &self.params.thumbnails {
            let asset_id = op.asset_id;
            let past_failed_job = interact!(conn, move |mut conn| {
                repository::failed_job::get_failed_thumbnail_job_for_asset(&mut conn, asset_id)
            })
            .in_current_span()
            .await??;
            if let Some(past_failed_job) = past_failed_job {
                let asset_path = interact!(conn, move |mut conn| {
                    repository::asset::get_asset_path_on_disk(&mut conn, asset_id)
                })
                .in_current_span()
                .await??
                .path_on_disk();
                let file = tokio::fs::File::open(&asset_path)
                    .in_current_span()
                    .await?
                    .try_into_std()
                    .unwrap();
                let current_hash = hash_file(file).in_current_span().await?;
                if current_hash == past_failed_job.file_hash {
                    debug!(
                        asset_id = ?asset_id,
                        "skipping thumbnail that failed in the past"
                    );
                    continue;
                }
            }

            let op_resolved = resolve(&op).in_current_span().await;
            let side_effect_results = match perform_side_effects_create_thumbnail(
                &self.storage,
                self.pool.clone(),
                op_resolved.clone(),
            )
            .in_current_span()
            .await
            {
                Err(err) => {
                    // same as above
                    // if things fail here it's not the asset's fault, so don't remember the fail
                    // in the database
                    failed.push(FailedThumbnail {
                        thumbnail: op.clone(),
                        err,
                    });
                    continue;
                }
                Ok(r) => r,
            };
            // if one thumbnail of op fails we discard the whole thing for now
            if !side_effect_results.failed.is_empty() {
                for (_thumbnail, report) in side_effect_results.failed {
                    failed.push(FailedThumbnail {
                        thumbnail: op.clone(),
                        err: report,
                    });
                }
                let failed_asset_ids: HashSet<AssetId> =
                    failed.iter().map(|f| f.thumbnail.asset_id).collect();
                for failed_asset_id in failed_asset_ids {
                    let asset_path = interact!(conn, move |mut conn| {
                        repository::asset::get_asset_path_on_disk(&mut conn, asset_id)
                    })
                    .in_current_span()
                    .await??
                    .path_on_disk();
                    let file = tokio::fs::File::open(&asset_path)
                        .await?
                        .try_into_std()
                        .unwrap();
                    let hash = hash_file(file).await?;
                    let insert_res = interact!(conn, move |mut conn| {
                        repository::failed_job::insert_failed_thumbnail_job(
                            &mut conn,
                            &FailedThumbnailJob {
                                asset_id: failed_asset_id,
                                file_hash: hash,
                                date: Utc::now(),
                            },
                        )
                    })
                    .in_current_span()
                    .await?;
                    if let Err(err) = insert_res {
                        error!(%err, "failed inserting FailedThumbnailJob");
                    }
                }
                continue;
            }
            let apply_result = apply_create_thumbnail(self.pool.clone(), op_resolved).await;
            if let Err(err) = apply_result {
                failed.push(FailedThumbnail {
                    thumbnail: op.clone(),
                    err,
                });
                continue;
            }
        }
        Ok(ThumbnailJobResult { failed })
    }
}

#[async_trait]
impl Job for ThumbnailJob {
    type Result = Result<ThumbnailJobResult>;

    fn start(self) -> JobHandle {
        let (tx, rx) = mpsc::channel::<JobProgress>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move {
            let r = self.run(tx, cancel_copy).await;
            JobResultType::Thumbnail(r)
        });
        let handle = JobHandle {
            progress_rx: rx,
            join_handle,
            cancel,
        };
        handle
    }
}
