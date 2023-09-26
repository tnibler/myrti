use async_trait::async_trait;
use eyre::{Report, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    catalog::operation::convert_image::ConvertImage,
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        storage::Storage,
    },
    model::repository::pool::DbPool,
};

pub struct ImageConversionJob {
    params: ImageConversionParams,
    pool: DbPool,
    storage: Storage,
}

#[derive(Debug, Clone)]
pub struct ImageConversionParams {
    pub ops: Vec<ConvertImage>,
}

#[derive(Debug)]
pub struct ImageConversionJobResult {
    pub failed: Vec<FailedImageConversion>,
}

#[derive(Debug)]
pub struct FailedImageConversion {
    pub op: ConvertImage,
    pub err: Report,
}

impl ImageConversionJob {
    pub fn new(
        params: ImageConversionParams,
        storage: Storage,
        pool: DbPool,
    ) -> ImageConversionJob {
        ImageConversionJob {
            params,
            pool,
            storage,
        }
    }

    async fn run(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> ImageConversionJobResult {
        todo!()
    }
}

#[async_trait]
impl Job for ImageConversionJob {
    type Result = ImageConversionJobResult;

    fn start(self) -> JobHandle {
        let (tx, rx) = mpsc::channel::<JobProgress>(1000);
        let cancel = CancellationToken::new();
        let cancel_copy = cancel.clone();
        let join_handle = tokio::spawn(async move {
            let r = self.run(tx, cancel_copy).await;
            JobResultType::ImageConversion(r)
        });
        let handle = JobHandle {
            progress_rx: rx,
            join_handle,
            cancel,
        };
        handle
    }
}
