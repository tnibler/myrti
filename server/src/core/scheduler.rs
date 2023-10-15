use std::collections::HashMap;

use camino::Utf8PathBuf as PathBuf;
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, instrument};

use crate::{
    catalog::{
        operation::{convert_image::ConvertImage, package_video::PackageVideo},
        rules,
    },
    core::job::JobType,
    eyre::Result,
    job::{
        image_conversion_job::{ImageConversionJob, ImageConversionParams},
        indexing_job::{IndexingJob, IndexingJobParams},
        thumbnail_job::{ThumbnailJob, ThumbnailJobParams},
        video_packaging_job::{VideoPackagingJob, VideoPackagingJobParams},
    },
    model::repository,
};

use super::{
    job::{Job, JobId, JobResultType},
    monitor::MonitorMessage,
    storage::Storage,
};

#[derive(Debug)]
pub enum SchedulerMessage {
    Timer,
    FileSystemChange { changed_files: Vec<PathBuf> },
    UserRequest(UserRequest),
    JobComplete { id: JobId, result: JobResultType },
    JobFailed { id: JobId },
    JobRegisteredWithMonitor { id: JobId, job_type: JobType },
    ConfigChange,
}

#[derive(Debug)]
pub enum UserRequest {
    ReindexAssetRoots { params: IndexingJobParams },
}

#[derive(Clone)]
pub struct Scheduler {
    cancel: CancellationToken,
    pub tx: mpsc::Sender<SchedulerMessage>,
}

impl Scheduler {
    pub fn start(
        monitor_tx: mpsc::Sender<MonitorMessage>,
        pool: SqlitePool,
        storage: Storage,
    ) -> Scheduler {
        let (tx, rx) = mpsc::channel::<SchedulerMessage>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let tx_copy = tx.clone();
        tokio::spawn(async move {
            let mut si = SchedulerImpl {
                events_tx: tx_copy,
                events_rx: rx,
                cancel,
                pool,
                monitor_tx,
                storage,
                running_jobs: HashMap::default(),
            };
            si.run().await;
        });
        Scheduler {
            cancel: cancel_copy,
            tx,
        }
    }

    pub async fn send(&self, msg: SchedulerMessage) -> Result<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}

struct SchedulerImpl {
    pub events_tx: mpsc::Sender<SchedulerMessage>,
    pub events_rx: mpsc::Receiver<SchedulerMessage>,
    pub cancel: CancellationToken,
    pool: SqlitePool,
    monitor_tx: mpsc::Sender<MonitorMessage>,
    storage: Storage,

    running_jobs: HashMap<JobId, JobType>,
}

impl SchedulerImpl {
    #[instrument(name = "event_loop", skip(self))]
    async fn run(&mut self) {
        info!("Scheduler starting");
        self.on_startup().await;
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Scheduler cancelled");
                    break;
                }
                Some(message) = self.events_rx.recv() => {
                    debug!(?message, "Received message");
                    match message {
                        SchedulerMessage::Timer => todo!(),
                        SchedulerMessage::FileSystemChange { changed_files: _ } => todo!(),
                        SchedulerMessage::UserRequest(request) => {
                            match request {
                                UserRequest::ReindexAssetRoots { params } => {
                                    self.index_asset_root(params).await;
                                },
                            }
                        },
                        SchedulerMessage::JobRegisteredWithMonitor { id, job_type } => {
                            assert!(
                                self.running_jobs.insert(id, job_type).is_none(),
                                "Attempting to insert already existing JobId into running_jobs"
                            );
                        }
                        SchedulerMessage::JobComplete {id, result }=> {
                            self.on_job_complete(id, result).await;
                            debug_assert!(self.running_jobs.remove(&id).is_some(), "Scheduler did not know about job that completed");
                        },
                        SchedulerMessage::JobFailed { id } => {
                            debug_assert!(self.running_jobs.remove(&id).is_some(), "Scheduler did not know about job that completed");
                        }
                        SchedulerMessage::ConfigChange => todo!(),
                    }
                }
            }
        }
    }

    async fn on_job_complete(&self, _job_id: JobId, _result: JobResultType) {
        self.start_any_job_if_required().await;
    }

    async fn on_startup(&self) {
        let asset_roots = repository::asset_root_dir::get_asset_roots(&self.pool)
            .await
            .expect("TODO how do we handle errors in scheduler");
        for asset_root in asset_roots {
            self.index_asset_root(IndexingJobParams {
                asset_root,
                sub_paths: None,
            })
            .await;
        }
    }

    async fn start_any_job_if_required(&self) -> bool {
        let thumbnail = self.thumbnail_if_required().await;
        let package_video = self.package_video_if_required().await;
        let convert_image = self.convert_image_if_required().await;
        return thumbnail || package_video || convert_image;
    }

    async fn thumbnail_if_required(&self) -> bool {
        // Only run one job of a given type at a time. In the future, we might want to run multiple,
        // in which case we'll need to compute the diff between the params of currently running
        // jobs and the output of rules::thumbnails_to_create
        let any_running_thumbnail_job = self
            .running_jobs
            .values()
            .any(|job_type| matches!(job_type, JobType::Thumbnail { params: _ }));
        if any_running_thumbnail_job {
            return false;
        }

        let thumbnails_to_create = rules::thumbnails_to_create(&self.pool).await.unwrap();
        if thumbnails_to_create.is_empty() {
            return false;
        }
        let params = ThumbnailJobParams {
            thumbnails: thumbnails_to_create,
        };
        let job = ThumbnailJob::new(params.clone(), self.pool.clone(), self.storage.clone());
        let handle = job.start();
        self.monitor_tx
            .send(MonitorMessage::AddJob {
                handle,
                ty: JobType::Thumbnail { params },
            })
            .await
            .unwrap();
        return true;
    }

    async fn package_video_if_required(&self) -> bool {
        // Only run one job of a given type at a time. In the future, we might want to run multiple,
        // in which case we'll need to compute the diff between the params of currently running
        // jobs and the output of rules::video_packaging_due
        let any_running_video_packaging_job = self
            .running_jobs
            .values()
            .any(|job_type| matches!(job_type, JobType::VideoPackaging { params: _ }));
        if any_running_video_packaging_job {
            return false;
        }

        let videos_to_package: Vec<PackageVideo> =
            rules::video_packaging_due(&self.pool).await.unwrap(); // TODO
        if videos_to_package.is_empty() {
            return false;
        }
        let params = VideoPackagingJobParams {
            tasks: videos_to_package,
        };
        let job = VideoPackagingJob::new(params.clone(), self.storage.clone(), self.pool.clone());
        let handle = job.start();
        self.monitor_tx
            .send(MonitorMessage::AddJob {
                handle,
                ty: JobType::VideoPackaging { params },
            })
            .await
            .unwrap();
        return true;
    }

    async fn convert_image_if_required(&self) -> bool {
        // Only run one job of a given type at a time. In the future, we might want to run multiple,
        // in which case we'll need to compute the diff between the params of currently running
        // jobs and the output of rules::video_packaging_due
        let any_running_image_convert_job = self
            .running_jobs
            .values()
            .any(|job_type| matches!(job_type, JobType::ImageConversion { params: _ }));
        if any_running_image_convert_job {
            return false;
        }
        let images_to_convert: Vec<ConvertImage> =
            rules::image_conversion_due(&self.pool).await.unwrap(); // TODO error handling
        if images_to_convert.is_empty() {
            return false;
        }
        let params = ImageConversionParams {
            ops: images_to_convert,
        };
        let job = ImageConversionJob::new(params.clone(), self.storage.clone(), self.pool.clone());
        let handle = job.start();
        self.monitor_tx
            .send(MonitorMessage::AddJob {
                handle,
                ty: JobType::ImageConversion { params },
            })
            .await
            .unwrap();
        return true;
    }

    async fn index_asset_root(&self, params: IndexingJobParams) -> bool {
        let asset_root_already_being_indexed =
            self.running_jobs.values().any(|job_type| match job_type {
                JobType::Indexing { params } if params.asset_root.id == params.asset_root.id => {
                    true
                }
                _ => false,
            });
        if asset_root_already_being_indexed {
            return false;
        }

        let job = IndexingJob::new(params.clone(), self.pool.clone());
        let handle = job.start();
        self.monitor_tx
            .send(MonitorMessage::AddJob {
                handle,
                ty: JobType::Indexing { params },
            })
            .await
            .unwrap();
        return true;
    }
}
