use eyre::{Context, Result, eyre};
use tokio::sync::mpsc;

use myrti_data::db::{DbPool, PooledDbConn};
use myrti_data::model::{
    AssetThumbnail, AssetThumbnailId, AssetType, FileId, ThumbnailFormat, ThumbnailType,
};
use myrti_data::{interact, repository};

use crate::catalog::image_conversion_target::ImageFormatTarget;
use crate::catalog::image_conversion_target::heif::AvifTarget;
use crate::core::queue_executor::{BatchQueueExecutor, JobError, JobId, JobProcessor};
use crate::processing;
use crate::processing::image::thumbnail::{
    ThumbnailParams, ThumbnailResult, generate_thumbnail, generate_video_thumbnail,
};
use crate::{
    catalog::storage_key,
    core::storage::{Storage, StorageProvider},
    processing::process_control::ProcessControlReceiver,
};

pub(super) type ImageProcessor = BatchQueueExecutor<ImageJob, Result<()>, ImageJobProcessor>;

#[derive(Clone)]
pub(super) struct ImageJobProcessor {
    pub db_pool: DbPool,
    pub storage: Storage,
}

#[derive(Debug, Clone)]
pub(super) enum ImageJob {
    CreateThumbnail {
        thumbnail_types: Vec<ThumbnailType>,
        formats: Vec<ThumbnailFormat>,
    },
    CreateThumbhash,
}

pub(super) type ImageProcessingMsg = (
    JobId,
    Result<<ImageJobProcessor as JobProcessor>::Output, JobError>,
);

impl JobProcessor for ImageJobProcessor {
    type Job = ImageJob;
    type Output = Result<()>;

    async fn process(
        &self,
        file_id: FileId,
        jobs: Vec<Self::Job>,
        mut control: super::queue_executor::JobControlRecv,
    ) -> Result<Self::Output, JobError> {
        let mut conn = self.db_pool.get().await?;
        for job in jobs {
            match job {
                ImageJob::CreateThumbnail {
                    thumbnail_types,
                    formats,
                } => {
                    let (process_control_send, mut process_control_recv) = mpsc::channel(1);
                    let result_fut = generate_asset_thumbnail(
                        &mut conn,
                        &self.storage,
                        file_id,
                        &thumbnail_types,
                        &formats,
                        &mut process_control_recv,
                    );
                    crate::core::queue_executor::run_process_loop(
                        result_fut,
                        &mut control,
                        process_control_send,
                    )
                    .await??;
                }
                ImageJob::CreateThumbhash => {
                    generate_thumbhash(&mut conn, &self.storage, file_id)
                        .await
                        .wrap_err("error generating asset thumbhash")?;
                }
            }
        }
        Ok(Ok(()))
    }
}

async fn generate_asset_thumbnail(
    conn: &mut PooledDbConn,
    storage: &Storage,
    file_id: FileId,
    thumbnail_types: &[ThumbnailType],
    formats: &[ThumbnailFormat],
    control_recv: &mut ProcessControlReceiver,
) -> Result<()> {
    let (in_path, file) = interact!(conn, move |conn| {
        let in_path = repository::asset::get_asset_path_on_disk(conn, file_id)?.path_on_disk();
        let file = repository::asset::get_asset_file(conn, file_id)?;
        Ok::<_, eyre::Report>((in_path, file))
    })
    .await??;

    for thumb_type in thumbnail_types.iter().copied() {
        let file_keys: Vec<(ThumbnailFormat, String)> = formats
            .iter()
            .copied()
            .map(|format| (format, storage_key::thumbnail(file_id, thumb_type, format)))
            .collect();

        let out_dimension = match thumb_type {
            ThumbnailType::SmallSquare => processing::image::OutDimension::Crop {
                width: 200,
                height: 200,
            },
            ThumbnailType::LargeOrigAspect => {
                processing::image::OutDimension::KeepAspect { height: 300 }
            }
        };
        let mut outputs = Vec::new();
        for (format, file_key) in &file_keys {
            let target = match format {
                ThumbnailFormat::Webp => ImageFormatTarget::WEBP,
                ThumbnailFormat::Avif => ImageFormatTarget::AVIF(AvifTarget {
                    quality: 50.try_into()?,
                    effort: 7.try_into()?,
                    ..Default::default()
                }),
            };
            outputs.push((storage.local_path(file_key).await?.unwrap(), target));
        }
        let thumbnail_params = ThumbnailParams {
            in_path: in_path.clone(),
            outputs,
            out_dimension,
        };
        let ThumbnailResult { actual_size } = match file.ty {
            AssetType::Image => generate_thumbnail(thumbnail_params).await,
            AssetType::Video => generate_video_thumbnail(thumbnail_params, control_recv).await,
        }?;
        for format in formats.iter().copied() {
            interact!(conn, move |conn| {
                repository::asset::insert_asset_thumbnail(
                    conn,
                    AssetThumbnail {
                        id: AssetThumbnailId(0),
                        file_id,
                        ty: thumb_type,
                        size: actual_size,
                        format,
                    },
                )
            })
            .await??;
        }
    }
    Ok(())
}

async fn generate_thumbhash(
    conn: &mut PooledDbConn,
    storage: &Storage,
    file_id: FileId,
) -> Result<()> {
    let file = interact!(conn, move |conn| {
        repository::asset::get_asset_file(conn, file_id)
    })
    .await??;
    let input_path = match file.ty {
        AssetType::Image => interact!(conn, move |conn| {
            repository::asset::get_asset_path_on_disk(conn, file_id)
        })
        .await??
        .path_on_disk(),
        AssetType::Video => {
            let thumbnail = interact!(conn, move |conn| {
                repository::asset::get_thumbnails_for_asset(conn, file_id)
            })
            .await??
            .into_iter()
            .find(|thumb| thumb.ty == ThumbnailType::LargeOrigAspect)
            .ok_or(eyre!(
                "can't generate thumbhash for video with no suitable thumbnail yet"
            ))?;
            let thumbnail_key = storage_key::thumbnail(file_id, thumbnail.ty, thumbnail.format);
            storage
                .local_path(&thumbnail_key)
                .await?
                .ok_or(eyre!("thumbnail must be local file"))?
        }
    };
    let thumbhash = crate::processing::image::thumbnail::generate_thumbhash(input_path).await?;
    interact!(conn, move |conn| {
        repository::asset::set_file_thumbhash(conn, file_id, &thumbhash)
    })
    .await??;
    Ok(())
}
