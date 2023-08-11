use std::{path::PathBuf, sync::Arc};

use super::{
    job::{Job, JobId, JobResultType},
    monitor::MonitorMessage,
    DataDirManager,
};
use crate::{
    catalog::rules,
    core::job::JobType,
    eyre::Result,
    job::{
        dash_segmenting_job::{DashSegmentingJob, DashSegmentingJobParams, VideoProcessingTask},
        indexing_job::{IndexingJob, IndexingJobParams},
        thumbnail_job::{ThumbnailJob, ThumbnailJobParams},
    },
    model::repository,
    model::AssetId,
};
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, instrument};

#[derive(Debug)]
pub enum SchedulerMessage {
    Timer,
    FileSystemChange { changed_files: Vec<PathBuf> },
    UserRequest(UserRequest),
    JobComplete { id: JobId, result: JobResultType },
    JobFailed { id: JobId },
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
    pub fn start(monitor_tx: mpsc::Sender<MonitorMessage>, pool: SqlitePool) -> Scheduler {
        let (tx, rx) = mpsc::channel::<SchedulerMessage>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let tx_copy = tx.clone();
        tokio::spawn(async move {
            let mut si = SchedulerImpl {
                events_tx: tx_copy,
                events_rx: rx,
                cancel,
                data_dir_manager: Arc::new(DataDirManager::new(pool.clone())),
                pool,
                monitor_tx,
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
    data_dir_manager: Arc<DataDirManager>,
}

impl SchedulerImpl {
    #[instrument(name = "event_loop", skip(self))]
    async fn run(&mut self) {
        info!("Scheduler starting");
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
                            self.transcode_and_package_if_required().await;
                            match request {
                                UserRequest::ReindexAssetRoots { params } => {
                                    self.queue_or_start_indexing(params).await;
                                },
                                _ => todo!()
                            }
                        },
                        SchedulerMessage::JobComplete {id, result }=> {
                            self.on_job_complete(id, result).await;
                        },
                        SchedulerMessage::JobFailed { id: _ } => {

                        }
                        SchedulerMessage::ConfigChange => todo!(),
                    }
                }
            }
        }
    }

    async fn on_job_complete(&self, _job_id: JobId, result: JobResultType) {
        match result {
            JobResultType::Indexing(_) => {
                self.thumbnail_if_required().await;
                self.dash_package_if_required().await;
            }
            JobResultType::Thumbnail(_) => {}
            JobResultType::DashSegmenting(_) => {}
        }
    }

    async fn thumbnail_if_required(&self) {
        let thumbnails_to_create = rules::thumbnails_to_create(&self.pool).await.unwrap();
        let params = ThumbnailJobParams {
            thumbnails: thumbnails_to_create,
        };
        let job = ThumbnailJob::new(
            params.clone(),
            self.data_dir_manager.clone(),
            self.pool.clone(),
        );
        let handle = job.start();
        self.monitor_tx
            .send(MonitorMessage::AddJob {
                handle,
                ty: JobType::Thumbnail { params },
            })
            .await
            .unwrap();
    }

    async fn dash_package_if_required(&self) {
        let videos_without_dash = repository::asset::get_video_assets_without_dash(&self.pool)
            .await
            .unwrap();
        debug!(?videos_without_dash);
        if !videos_without_dash.is_empty() {
            let tasks: Vec<VideoProcessingTask> = videos_without_dash
                .iter()
                .map(|asset| VideoProcessingTask::DashPackageOnly { asset_id: asset.id })
                .collect();
            let params = DashSegmentingJobParams { tasks };
            let job = DashSegmentingJob::new(
                params.clone(),
                self.data_dir_manager.clone(),
                self.pool.clone(),
            );
            let handle = job.start();
            self.monitor_tx
                .send(MonitorMessage::AddJob {
                    handle,
                    ty: JobType::DashSegmenting { params },
                })
                .await
                .unwrap();
        }
    }

    async fn transcode_and_package_if_required(&self) {
        // priority:
        //  - videos with no representation in acceptable codec
        //  - videos with any representation from their quality ladder missing
        //    (hightest qualities come first)
        // For now, scan through all videos to check.
        // Later, set a flag if all required transcoding has been done
        // and clear the flag when the config (quality ladder, acceptable codecs)
        // change and recheck

        let acceptable_codecs = ["h264", "av1", "vp9"];
        let no_good_reprs = repository::asset::get_video_assets_with_no_acceptable_repr(
            &self.pool,
            acceptable_codecs.into_iter(),
        )
        .await
        .unwrap();
        // transcode no_good_reprs into target codec first
        // we want DashPackagingJob to either only package, or transcode and then package
        // by adding an Option<EncodingTarget> to every param

        // if all videos have at least one good repr:
        //   for every rung in quality_levels starting from the highest
        //     for every video, ql = quality_ladder(video):
        //       if rung in ql and no repr for video at that rung:
        //         transcode to that rung
    }

    async fn queue_or_start_indexing(&mut self, params: IndexingJobParams) {
        // // always starting job, no queue yet
        let job = IndexingJob::new(params.clone(), self.pool.clone());
        let handle = job.start();
        self.monitor_tx
            .send(MonitorMessage::AddJob {
                handle,
                ty: JobType::Indexing { params },
            })
            .await
            .unwrap();
    }
}
