use std::{path::PathBuf, sync::Arc};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        DataDirManager,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetId,
    },
    processing::video::{
        dash_package::{shaka_package, RepresentationInput, RepresentationType},
        probe_video,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashSegmentingJobParams {
    pub asset_id: AssetId,
}

pub struct DashSegmentingJob {
    params: DashSegmentingJobParams,
    data_dir_manager: Arc<DataDirManager>,
    pool: DbPool,
}

#[derive(Debug)]
pub struct DashSegmentingJobResult {}

impl DashSegmentingJob {
    pub fn new(
        params: DashSegmentingJobParams,
        data_dir_manager: Arc<DataDirManager>,
        pool: DbPool,
    ) -> DashSegmentingJob {
        DashSegmentingJob {
            pool,
            data_dir_manager,
            params,
        }
    }

    async fn run(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> DashSegmentingJobResult {
        // TODO check it's actually a video
        let asset_path =
            repository::asset::get_asset_path_on_disk(&self.pool, self.params.asset_id)
                .await
                .unwrap();
        let probe = probe_video(&asset_path.path_on_disk()).await.unwrap();
        dbg!(&probe);
        let reprs = &[
            RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Video,
            },
            RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Audio,
            },
        ];
        shaka_package(reprs, &PathBuf::from("dashout"))
            .await
            .unwrap();
        DashSegmentingJobResult {}
    }
}

impl Job for DashSegmentingJob {
    type Result = DashSegmentingJobResult;

    fn start(self) -> JobHandle {
        let (tx, rx) = mpsc::channel::<JobProgress>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move {
            let r = self.run(tx, cancel_copy).await;
            JobResultType::DashSegmenting(r)
        });
        let handle: JobHandle = JobHandle {
            progress_rx: rx,
            join_handle,
            cancel,
        };
        handle
    }
}
