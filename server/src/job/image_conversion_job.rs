use async_trait::async_trait;
use eyre::{Report, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{
    catalog::operation::convert_image::{
        apply_convert_image, perform_side_effects_convert_image, preallocate_dummy_image_repr_rows,
        ConvertImage,
    },
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
        // TODO send progress updates
        status_tx
            .send(JobProgress {
                percent: None,
                description: "".to_string(),
            })
            .await
            .unwrap();
        let mut failed: Vec<FailedImageConversion> = Vec::default();
        for op in &self.params.ops {
            // TODO check past failed jobs
            // let past_failed_job =
            //     repository::failed_job::get_failed_image_conversion_job_for_asset(&self.pool, op.asset_id)
            //         .await?;
            let reserved = match preallocate_dummy_image_repr_rows(&self.pool, op).await {
                Err(err) => {
                    failed.push(FailedImageConversion {
                        op: op.clone(),
                        err,
                    });
                    continue;
                }
                Ok(r) => r,
            };
            let size =
                match perform_side_effects_convert_image(&reserved, op, &self.pool, &self.storage)
                    .in_current_span()
                    .await
                {
                    Err(err) => {
                        failed.push(FailedImageConversion {
                            op: op.clone(),
                            err,
                        });
                        continue;
                    }
                    Ok(r) => r,
                };
            let apply_result = apply_convert_image(&self.pool, op, &reserved, size)
                .in_current_span()
                .await;
            if let Err(err) = apply_result {
                failed.push(FailedImageConversion {
                    op: op.clone(),
                    err,
                });
                continue;
            }
        }
        ImageConversionJobResult { failed }
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
