use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use eyre::{Context, ErrReport, Result};
use rayon::prelude::*;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, Instrument};

use crate::{
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType, JobStatus},
        DataDirManager, NewResourceFile,
    },
    model::repository::{self, pool::DbPool},
    model::{
        AssetBase, AssetId, AssetPathOnDisk, AssetThumbnails, AssetType, ResourceFileResolved,
    },
    processing::{
        self,
        image::{generate_thumbnail, ThumbnailParams},
    },
};

pub struct ThumbnailJob {
    params: ThumbnailJobParams,
    data_dir_manager: Arc<DataDirManager>,
    pool: DbPool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailJobParams {
    pub asset_ids: Vec<AssetId>,
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

    #[instrument(name = "Run ThumbnailJob", skip(self, status_tx))]
    async fn run(self, status_tx: mpsc::Sender<JobStatus>) -> Result<ThumbnailResult> {
        status_tx
            .send(JobStatus::Running(JobProgress {
                percent: None,
                description: "".to_string(),
            }))
            .await
            .unwrap();
        let assets: Result<Vec<AssetThumbnails>> =
            repository::asset::get_assets_with_missing_thumbnail(&self.pool, None)
                .in_current_span()
                .await;
        let assets = assets.wrap_err("could not get Assets from db")?;
        let tasks_to_do = gather_thumbnail_tasks(&assets, self.data_dir_manager.as_ref()).await;
        if !tasks_to_do.failed.is_empty() {
            error!("failed to create some thumbnail tasks");
            // TODO
        }
        let result = generate_thumbnails(tasks_to_do.tasks, &self.pool).await;
        for failed in result.failed.iter() {
            error!(
                "Failed to generate thumbnail for asset {}: {}",
                failed.task.asset_id,
                failed.error.to_string()
            );
        }
        Ok(result)
    }
}

#[async_trait]
impl Job for ThumbnailJob {
    type Result = Result<ThumbnailResult>;

    fn start(self) -> JobHandle {
        let (tx, rx) = mpsc::channel::<JobStatus>(1000);
        let cancel = CancellationToken::new();
        let _cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move {
            let r = self.run(tx).await;
            JobResultType::Thumbnail(r)
        });
        let handle = JobHandle {
            status_rx: rx,
            join_handle,
            cancel,
        };
        handle
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ThumbnailType {
    SmallSquare,
    LargeOrigAspect,
}

#[derive(Debug, Clone)]
pub struct ThumbnailTasksForAsset {
    pub asset_id: AssetId,
    pub asset_ty: AssetType,
    pub tasks: Vec<ThumbnailTask>,
}

#[derive(Debug, Clone)]
pub struct ThumbnailTask {
    pub ty: ThumbnailType,
    pub jpg_file: NewResourceFile,
    pub webp_file: NewResourceFile,
}

struct GatherThumbnailTasksResult {
    pub tasks: Vec<ThumbnailTasksForAsset>,
    pub failed: Vec<(AssetId, eyre::Report)>,
}

async fn gather_thumbnail_tasks(
    assets: &[AssetThumbnails],
    data_dir_manager: &DataDirManager,
) -> GatherThumbnailTasksResult {
    let mut failed = Vec::<(AssetId, eyre::Report)>::new();
    let mut tasks = Vec::<ThumbnailTasksForAsset>::new();
    for asset in assets {
        let mut task_type_and_filenames: Vec<(ThumbnailType, String, String)> = vec![];
        if asset.thumb_large_orig_jpg.is_none() || asset.thumb_large_orig_webp.is_none() {
            task_type_and_filenames.push((
                ThumbnailType::LargeOrigAspect,
                format!("{}.jpg", asset.id.0.to_string()),
                format!("{}.webp", asset.id.0.to_string()),
            ));
        }
        if asset.thumb_small_square_jpg.is_none() || asset.thumb_small_square_webp.is_none() {
            task_type_and_filenames.push((
                ThumbnailType::SmallSquare,
                format!("{}_sm.jpg", asset.id.0.to_string()),
                format!("{}_sm.webp", asset.id.0.to_string()),
            ));
        }
        let mut tasks_for_asset: Vec<ThumbnailTask> = vec![];
        for (thumbnail_type, jpg_name, webp_name) in task_type_and_filenames.into_iter() {
            let jpg_file = data_dir_manager.new_thumbnail_file(&jpg_name).await;
            let webp_file = data_dir_manager.new_thumbnail_file(&webp_name).await;
            if let Err(e) = jpg_file {
                failed.push((asset.id, e.wrap_err("failed to create new ResourceFile")));
                continue;
            }
            if let Err(e) = webp_file {
                failed.push((asset.id, e.wrap_err("failed to create new ResourceFile")));
                continue;
            }
            tasks_for_asset.push(ThumbnailTask {
                ty: thumbnail_type,
                jpg_file: jpg_file.unwrap(),
                webp_file: webp_file.unwrap(),
            });
        }
        tasks.push(ThumbnailTasksForAsset {
            asset_id: asset.id,
            asset_ty: asset.ty,
            tasks: tasks_for_asset,
        });
    }
    GatherThumbnailTasksResult { failed, tasks }
}

#[derive(Debug)]
pub struct ThumbnailResult {
    pub succeeded: Vec<ThumbnailTasksForAsset>,
    pub failed: Vec<FailedThumbnailTask>,
}

#[derive(Debug)]
pub struct ThumbnailTaskResult {
    task: ThumbnailTasksForAsset,
    result: Result<()>,
}

#[derive(Debug)]
pub struct FailedThumbnailTask {
    task: ThumbnailTasksForAsset,
    error: eyre::Report,
}

async fn generate_thumbnails(tasks: Vec<ThumbnailTasksForAsset>, pool: &DbPool) -> ThumbnailResult {
    let mut image_tasks = Vec::<ThumbnailTasksForAsset>::new();
    let mut videos = Vec::<ThumbnailTasksForAsset>::new();
    for task in tasks.into_iter() {
        match task.asset_ty {
            AssetType::Image => image_tasks.push(task),
            AssetType::Video => videos.push(task),
        }
    }
    info!(
        "Generating thumbnails for {} images, {} videos",
        image_tasks.len(),
        videos.len()
    );
    let mut failed = Vec::<FailedThumbnailTask>::new();
    let mut succeeded: Vec<ThumbnailTasksForAsset> = vec![];
    for tasks in image_tasks.into_iter() {
        let asset_path =
            match repository::asset::get_asset_path_on_disk(&pool, tasks.asset_id).await {
                Ok(AssetPathOnDisk {
                    path_in_asset_root,
                    asset_root_path,
                }) => asset_root_path.join(path_in_asset_root),
                Err(e) => {
                    failed.push(FailedThumbnailTask {
                        task: tasks,
                        error: e.wrap_err("could not get asset path on disk from db"),
                    });
                    continue;
                }
            };
        for task_for_asset in tasks.tasks.iter() {
            let (tx, rx) = oneshot::channel();
            let params = ThumbnailParams {
                in_path: (&asset_path).clone(),
                out_paths: vec![
                    task_for_asset.jpg_file.path_on_disk(),
                    task_for_asset.webp_file.path_on_disk(),
                ],
                out_dimension: match task_for_asset.ty {
                    ThumbnailType::SmallSquare => processing::image::OutDimension::Crop {
                        width: 200,
                        height: 200,
                    },
                    ThumbnailType::LargeOrigAspect => {
                        processing::image::OutDimension::KeepAspect { width: 300 }
                    }
                },
            };
            tokio::task::spawn_blocking(move || {
                // TODO handle errors here
                generate_thumbnail(params).unwrap();
                tx.send(()).unwrap();
            })
            .await
            .unwrap();
            rx.await.unwrap();
            // TODO handle errors
            repository::resource_file::insert_new_resource_file(
                pool,
                task_for_asset.jpg_file.clone(),
            )
            .await
            .unwrap();
            repository::resource_file::insert_new_resource_file(
                pool,
                task_for_asset.webp_file.clone(),
            )
            .await
            .unwrap();
        }
        succeeded.push(tasks);
    }
    ThumbnailResult { succeeded, failed }
}
