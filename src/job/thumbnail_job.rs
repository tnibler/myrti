use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use eyre::{Context, Report, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, Instrument};

use crate::{
    catalog::{
        operation::create_thumbnail::{
            apply_create_thumbnail, perform_side_effects_create_thumbnail, CreateThumbnail,
            ThumbnailToCreate,
        },
        PathInResourceDir, ResolvedNewResourcePath, ResolvedResourcePath,
    },
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        DataDirManager,
    },
    model::repository::pool::DbPool,
};

pub struct ThumbnailJob {
    params: ThumbnailJobParams,
    data_dir_manager: Arc<DataDirManager>,
    pool: DbPool,
}

#[derive(Debug, Clone)]
pub struct ThumbnailJobParams {
    pub thumbnails: Vec<CreateThumbnail<PathInResourceDir>>,
}

#[derive(Debug)]
pub struct FailedThumbnail {
    pub thumbnail: CreateThumbnail<PathInResourceDir>,
    pub err: Report,
}

#[derive(Debug)]
pub struct ThumbnailJobResult {
    pub failed: Vec<FailedThumbnail>,
}

impl ThumbnailJob {
    pub fn new(
        params: ThumbnailJobParams,
        data_dir_manager: Arc<DataDirManager>,
        pool: DbPool,
    ) -> ThumbnailJob {
        ThumbnailJob {
            params,
            data_dir_manager,
            pool,
        }
    }

    #[instrument(name = "ThumbnailJob", skip(self, status_tx))]
    async fn run(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> Result<ThumbnailJobResult> {
        // TODO send progress updates
        status_tx
            .send(JobProgress {
                percent: None,
                description: "".to_string(),
            })
            .await
            .unwrap();
        let mut failed: Vec<FailedThumbnail> = Vec::default();
        for op in self.params.thumbnails {
            let thumbs_resolved = match resolve(&op.thumbnails, &self.data_dir_manager).await {
                Ok(t) => t,
                Err(err) => {
                    failed.push(FailedThumbnail { thumbnail: op, err });
                    continue;
                }
            };
            let op_resolved = CreateThumbnail {
                asset_id: op.asset_id,
                thumbnails: thumbs_resolved,
            };
            let side_effect_results =
                perform_side_effects_create_thumbnail(&self.pool, &op_resolved)
                    .await
                    .unwrap();
            apply_create_thumbnail(&self.pool, &op_resolved)
                .await
                .unwrap();
        }
        return Ok(ThumbnailJobResult { failed });

        async fn resolve(
            thumbs: &[ThumbnailToCreate<PathInResourceDir>],
            data_dir_manager: &DataDirManager,
        ) -> Result<Vec<ThumbnailToCreate<ResolvedResourcePath>>> {
            let mut thumbnails_to_create: Vec<ThumbnailToCreate<ResolvedResourcePath>> =
                Vec::default();
            for thumb in thumbs {
                let avif_file = data_dir_manager
                    .new_thumbnail_file(&thumb.avif_file.0)
                    .await
                    .wrap_err("could not create new thumbnail resource file")?;
                let avif_path: ResolvedResourcePath =
                    ResolvedResourcePath::New(ResolvedNewResourcePath {
                        data_dir_id: avif_file.data_dir_id,
                        path_in_data_dir: avif_file.path_in_data_dir,
                    });
                let webp_file = data_dir_manager
                    .new_thumbnail_file(&thumb.webp_file.0)
                    .await
                    .wrap_err("could not create new thumbnail resource file")?;
                let webp_path: ResolvedResourcePath =
                    ResolvedResourcePath::New(ResolvedNewResourcePath {
                        data_dir_id: webp_file.data_dir_id,
                        path_in_data_dir: webp_file.path_in_data_dir,
                    });
                let thumbnail_to_create: ThumbnailToCreate<ResolvedResourcePath> =
                    ThumbnailToCreate {
                        ty: thumb.ty,
                        webp_file: webp_path,
                        avif_file: avif_path,
                    };
                thumbnails_to_create.push(thumbnail_to_create);
            }
            Ok(thumbnails_to_create)
        }
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
