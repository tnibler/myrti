use eyre::{Context, Result, eyre};
use tokio::sync::mpsc;

use myrti_data::db::{DbPool, PooledDbConn};
use myrti_data::model::{
    AssetThumbnail, AssetThumbnailId, AssetType, FileId, ImageRepresentation,
    ImageRepresentationId, Size, ThumbnailFormat, ThumbnailType,
};
use myrti_data::{interact, repository};

use crate::catalog::image_conversion_target::heif::AvifTarget;
use crate::catalog::image_conversion_target::{ImageFormatTarget, image_format_name};
use crate::catalog::operation::convert_image::ConvertImage;
use crate::catalog::operation::package_video::{PackageVideo, do_package_video};
use crate::config::Config;
use crate::core::queue_executor::{
    BatchQueueExecutor, JobError, JobId, JobProcessor, run_process_loop,
};
use crate::core::storage::StorageCommandOutput;
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
    pub config: Config,
}

#[derive(Debug, Clone)]
pub(super) enum ImageJob {
    CreateThumbnail {
        thumbnail_types: Vec<ThumbnailType>,
        formats: Vec<ThumbnailFormat>,
    },
    CreateThumbhash,
    ConvertImage(ConvertImage),
    PackageVideo(PackageVideo),
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
        mut control_recv: super::queue_executor::JobControlRecv,
    ) -> Result<Self::Output, JobError> {
        let mut conn = self.db_pool.get().await?;
        for job in jobs {
            match job {
                ImageJob::CreateThumbnail {
                    thumbnail_types,
                    formats,
                } => {
                    tracing::trace!(?file_id, ?thumbnail_types, ?formats, "create thumbnail");
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
                        &mut control_recv,
                        process_control_send,
                    )
                    .await??;
                }
                ImageJob::CreateThumbhash => {
                    tracing::trace!(?file_id, "create thumbhash");
                    generate_thumbhash(&mut conn, &self.storage, file_id)
                        .await
                        .wrap_err("error generating asset thumbhash")?;
                }
                ImageJob::ConvertImage(convert_op) => {
                    tracing::trace!(?file_id, ?convert_op, "converting image");
                    convert_image(&mut conn, &self.storage, convert_op)
                        .await
                        .wrap_err("error converting image")?;
                }
                ImageJob::PackageVideo(package_op) => {
                    tracing::trace!(?file_id, ?package_op, "packaging video");
                    let (process_control_send, process_control_recv) =
                        tokio::sync::mpsc::channel(1);
                    let result_fut = do_package_video(
                        &self.db_pool,
                        &self.storage,
                        package_op.clone(),
                        self.config.bin_paths.as_ref(),
                        process_control_recv,
                    );
                    run_process_loop(result_fut, &mut control_recv, process_control_send)
                        .await
                        .wrap_err("error running video packaging job")??;
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

async fn convert_image(conn: &mut PooledDbConn, storage: &Storage, op: ConvertImage) -> Result<()> {
    let command_out_file = storage.new_command_out_file(&op.output_file_key).await?;
    let file_id = op.file_id;
    let (file, asset_path) = interact!(conn, move |conn| {
        // FIXME (low) unnecessarily querying same row twice
        let file = repository::asset::get_asset_file(conn, file_id)?;
        let asset_path = repository::asset::get_asset_path_on_disk(conn, file_id)?;
        Ok((file, asset_path))
    })
    .await??;

    let out_path = command_out_file.path().to_owned();
    let input_path = asset_path.path_on_disk();
    let target = op.target.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    rayon::spawn(move || {
        let res = processing::image::convert_image(&input_path, &out_path, &target);
        tx.send(res).expect("receiver thread should not have died");
    });
    let scaled_size = rx
        .await
        .wrap_err("error in image conversion task")?
        .wrap_err("error converting image")?
        .map(|processing_size| Size {
            width: processing_size.width,
            height: processing_size.height,
        });
    let final_size = scaled_size.unwrap_or(file.size);
    let file_size = command_out_file.size().await?;
    command_out_file.flush_to_storage().await?;

    let image_representation = ImageRepresentation {
        id: ImageRepresentationId(0),
        file_id: op.file_id,
        format_name: image_format_name(&op.target.format).to_owned(),
        file_key: op.output_file_key.clone(),
        file_size: file_size.try_into().unwrap(),
        width: final_size.width,
        height: final_size.height,
    };
    interact!(conn, move |conn| {
        repository::representation::insert_image_representation(conn, &image_representation)
            .wrap_err("error inserting image representation")
    })
    .await??;
    Ok(())
}
