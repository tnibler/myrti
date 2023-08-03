use async_trait::async_trait;
use eyre::{Context, ErrReport, Result};
use rayon::prelude::*;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::TempPath;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, Instrument};

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
        video::create_snapshot,
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

    #[instrument(name = "ThumbnailJob", skip(self, status_tx))]
    async fn run(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> Result<ThumbnailResult> {
        status_tx
            .send(JobProgress {
                percent: None,
                description: "".to_string(),
            })
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
        let result = generate_thumbnails(tasks_to_do.tasks, &self.pool, cancel).await;
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

// TODO this is all way too convoluted
// TODO also probably parallelize to an extent, don't wait sequentially for every job to complete

#[instrument("Gather thumbnail tasks", skip(assets, data_dir_manager))]
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

#[instrument(name = "Generate Thumbnails", skip(tasks, pool, cancel))]
async fn generate_thumbnails(
    tasks: Vec<ThumbnailTasksForAsset>,
    pool: &DbPool,
    cancel: CancellationToken,
) -> ThumbnailResult {
    let mut image_tasks = Vec::<ThumbnailTasksForAsset>::new();
    let mut video_tasks = Vec::<ThumbnailTasksForAsset>::new();
    for task in tasks.into_iter() {
        match task.asset_ty {
            AssetType::Image => image_tasks.push(task),
            AssetType::Video => video_tasks.push(task),
        }
    }
    info!(
        "Generating thumbnails for {} images, {} videos",
        image_tasks.len(),
        video_tasks.len()
    );
    let mut failed = Vec::<FailedThumbnailTask>::new();
    let mut succeeded: Vec<ThumbnailTasksForAsset> = vec![];
    for tasks in image_tasks.into_iter() {
        if cancel.is_cancelled() {
            break;
        }
        let asset_path =
            match repository::asset::get_asset_path_on_disk(&pool, tasks.asset_id).await {
                Ok(AssetPathOnDisk {
                    id,
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
            if cancel.is_cancelled() {
                break;
            }
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
            rayon::spawn(move || {
                // TODO handle errors here
                generate_thumbnail(params).unwrap();
                tx.send(()).unwrap();
            });
            rx.await.unwrap();
            let mut tx = pool
                .begin()
                .await
                .wrap_err("could not begin db transaction")
                .unwrap();
            // TODO handle errors
            let jpg_id = repository::resource_file::insert_new_resource_file(
                &mut tx,
                task_for_asset.jpg_file.clone(),
            )
            .await
            .unwrap();
            let webp_id = repository::resource_file::insert_new_resource_file(
                &mut tx,
                task_for_asset.webp_file.clone(),
            )
            .await
            .unwrap();
            match task_for_asset.ty {
                // TODO handle errors
                ThumbnailType::SmallSquare => {
                    repository::asset::set_asset_small_thumbnails(
                        tx.as_mut(),
                        tasks.asset_id,
                        jpg_id,
                        webp_id,
                    )
                    .await
                    .unwrap();
                }
                ThumbnailType::LargeOrigAspect => {
                    repository::asset::set_asset_large_thumbnails(
                        tx.as_mut(),
                        tasks.asset_id,
                        jpg_id,
                        webp_id,
                    )
                    .await
                    .unwrap();
                }
            }
            tx.commit()
                .await
                .wrap_err("could not commit db transaction")
                .unwrap();
        }
        succeeded.push(tasks);
    }
    let mut video_result = generate_video_thumbnails(video_tasks, pool, cancel)
        .in_current_span()
        .await;
    succeeded.append(&mut video_result.succeeded);
    failed.append(&mut video_result.failed);
    ThumbnailResult { succeeded, failed }
}

#[instrument(name = "Generate video thumbnails", skip(tasks, pool, cancel))]
pub async fn generate_video_thumbnails(
    tasks: Vec<ThumbnailTasksForAsset>,
    pool: &DbPool,
    cancel: CancellationToken,
) -> ThumbnailResult {
    let mut failed = Vec::<FailedThumbnailTask>::new();
    let mut succeeded: Vec<ThumbnailTasksForAsset> = vec![];
    for tasks in tasks.into_iter() {
        if cancel.is_cancelled() {
            break;
        }
        let asset_path = match repository::asset::get_asset_path_on_disk(&pool, tasks.asset_id)
            .in_current_span()
            .await
        {
            Ok(AssetPathOnDisk {
                id,
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
        let snapshot_dir = PathBuf::from("/tmp/mediathingy");
        // tokio::fs::remove_dir_all(&snapshot_dir).await.unwrap();
        // tokio::fs::create_dir_all(&snapshot_dir).await.unwrap();
        let snapshot_dir = match tempfile::tempdir().wrap_err("could not create temp directory") {
            Ok(d) => d,
            Err(e) => {
                failed.push(FailedThumbnailTask {
                    task: tasks,
                    error: e,
                });
                continue;
            }
        };
        let snapshot_path = snapshot_dir
            .path()
            .join(format!("{}.webp", tasks.asset_id.0));
        if let Err(e) = create_snapshot(&asset_path, &snapshot_path)
            .in_current_span()
            .await
            .wrap_err("could not take video snapshot")
        {
            failed.push(FailedThumbnailTask {
                task: tasks,
                error: e,
            });
            continue;
        }

        for task_for_asset in tasks.tasks.iter() {
            if cancel.is_cancelled() {
                break;
            }
            let (tx, rx) = oneshot::channel();
            let params = ThumbnailParams {
                in_path: snapshot_path.to_path_buf(),
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
            rayon::spawn(move || {
                // TODO handle errors here
                generate_thumbnail(params).unwrap();
                tx.send(()).unwrap();
            });
            rx.in_current_span().await.unwrap();
            let mut tx = pool
                .begin()
                .in_current_span()
                .await
                .wrap_err("could not begin db transaction")
                .unwrap();
            // TODO handle errors
            let jpg_id = repository::resource_file::insert_new_resource_file(
                &mut tx.as_mut(),
                task_for_asset.jpg_file.clone(),
            )
            .in_current_span()
            .await
            .unwrap();
            let webp_id = repository::resource_file::insert_new_resource_file(
                &mut tx.as_mut(),
                task_for_asset.webp_file.clone(),
            )
            .in_current_span()
            .await
            .unwrap();
            match task_for_asset.ty {
                // TODO handle errors
                ThumbnailType::SmallSquare => {
                    repository::asset::set_asset_small_thumbnails(
                        &mut tx.as_mut(),
                        tasks.asset_id,
                        jpg_id,
                        webp_id,
                    )
                    .in_current_span()
                    .await
                    .unwrap();
                }
                ThumbnailType::LargeOrigAspect => {
                    repository::asset::set_asset_large_thumbnails(
                        &mut tx.as_mut(),
                        tasks.asset_id,
                        jpg_id,
                        webp_id,
                    )
                    .in_current_span()
                    .await
                    .unwrap();
                }
            }
            tx.commit()
                .in_current_span()
                .await
                .wrap_err("could not commit db transaction")
                .unwrap();
        }
        succeeded.push(tasks);
    }
    ThumbnailResult { succeeded, failed }
}
