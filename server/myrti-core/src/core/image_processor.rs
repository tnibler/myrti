use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::os::unix::fs::MetadataExt;
use std::sync::{Arc, Mutex};

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
use crate::config::Config;
use crate::core::job_control::{
    JobControl, JobControlMsg, JobControlRecv, JobError, JobId, new_job_control, run_process_loop,
};
use crate::processing;
use crate::processing::image::thumbnail::{
    ThumbnailParams, ThumbnailResult, generate_thumbnail, generate_video_thumbnail,
};
use crate::{
    catalog::storage_key, core::storage::Storage,
    processing::process_control::ProcessControlReceiver,
};

#[derive(Clone)]
struct RunningJob {
    control: JobControl,
    job: ImageJob,
}

#[derive(Clone)]
pub(super) struct ImageJobProcessor {
    max_running: usize,
    max_queued: usize,
    queue: VecDeque<(FileId, VecDeque<(JobId, ImageJob)>)>,
    n_queued: usize,
    accepting_new: bool,
    // back up to scheduler
    result_send: Option<mpsc::Sender<ImageProcessingMsg>>,
    running_jobs: HashMap<JobId, RunningJob>,
    next_job_id: u64,

    db_pool: DbPool,
    storage: Storage,
    config: Arc<Mutex<Config>>,
}

#[derive(Debug, Clone)]
pub(super) enum ImageJob {
    CreateThumbnail {
        thumbnail_types: Vec<ThumbnailType>,
        formats: Vec<ThumbnailFormat>,
    },
    CreateThumbhash,
    ConvertImage(ConvertImage),
}

pub(super) type ImageProcessingMsg = (JobId, Result<Result<()>, JobError>);

impl ImageJobProcessor {
    pub fn new(
        max_running: NonZeroUsize,
        max_queued: NonZeroUsize,
        result_send: mpsc::Sender<ImageProcessingMsg>,
        db_pool: DbPool,
        storage: Storage,
        config: Arc<Mutex<Config>>,
    ) -> Self {
        Self {
            max_running: max_running.get(),
            max_queued: max_queued.get(),
            queue: Default::default(),
            n_queued: 0,
            accepting_new: true,
            result_send: Some(result_send),
            running_jobs: Default::default(),
            next_job_id: 0,
            db_pool,
            storage,
            config,
        }
    }

    pub fn enqueue_job(&mut self, file_id: FileId, job: ImageJob) -> Result<JobId, ImageJob> {
        if !self.accepting_new || self.n_queued >= self.max_queued {
            tracing::trace!(?file_id, ?job, "not accepting new image job");
            return Err(job);
        }
        let job_id = self.new_job_id();

        if let Some((_file_id, jobs)) = self
            .queue
            .iter_mut()
            .rfind(|(queued_file_id, _jobs)| *queued_file_id == file_id)
        {
            tracing::trace!(?file_id, "adding image job to existing batch for file");
            jobs.push_back((job_id, job));
            self.n_queued += 1;
        } else if self.running_jobs.len() < self.max_running {
            tracing::trace!(?file_id, "starting image job immediately");
            self.start_job(file_id, job_id, job);
        } else {
            tracing::trace!(?file_id, "enqueuing image job");
            self.queue
                .push_back((file_id, VecDeque::from([(job_id, job)])));
            self.n_queued += 1;
        }
        debug_assert_eq!(
            self.n_queued,
            self.queue.iter().map(|(_, jobs)| jobs.len()).sum::<usize>()
        );
        Ok(job_id)
    }

    pub fn try_dequeue(&mut self) {
        let mut dequeued_count = 0;
        while self.accepting_new && self.running_jobs.len() < self.max_running {
            let deq = self
                .queue
                .front_mut()
                .map(|(file_id, jobs)| (*file_id, jobs.pop_front()));
            match deq {
                Some((file_id, Some((job_id, job)))) => {
                    self.n_queued -= 1;
                    dequeued_count += 1;
                    self.start_job(file_id, job_id, job);
                }
                Some((_, None)) => {
                    unreachable!(
                        "empty lists are never pushed to the queue and immediately removed after dequeueing"
                    );
                }
                None => break,
            }
            self.queue.pop_front_if(|(_, jobs)| jobs.is_empty());
        }
        debug_assert_eq!(
            self.n_queued,
            self.queue.iter().map(|(_, jobs)| jobs.len()).sum::<usize>()
        );
        let remaining_queued = self.n_queued;
        tracing::trace!(dequeued_count, remaining_queued, "try_dequeue");
    }

    pub fn pause_all(&mut self) {
        tracing::debug!("pausing all");
        self.accepting_new = false;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Pause);
        }
    }

    pub fn resume_all(&mut self) {
        tracing::debug!("resuming all");
        self.accepting_new = true;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Resume);
        }
    }

    pub fn cancel_all(&mut self) {
        let n_jobs = self.running_jobs.len();
        tracing::debug!(n_jobs, "cancelling all");
        self.accepting_new = false;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Cancel);
        }
    }

    pub fn shutdown(&mut self) {
        self.cancel_all();
        self.result_send = None;
    }

    pub fn on_job_finished(&mut self, job_id: JobId) {
        tracing::trace!(?job_id, "image job finished");
        self.running_jobs
            .remove(&job_id)
            .expect("tried removing non existing job from running_jobs");
        self.try_dequeue();
    }

    pub fn queued(&self) -> usize {
        self.n_queued
    }

    fn start_job(&mut self, file_id: FileId, job_id: JobId, job: ImageJob) {
        assert!(self.running_jobs.len() < self.max_running);
        tracing::trace!(?job_id, ?file_id, "starting job");
        let (control_send, control_recv) = new_job_control();
        let result_send = self.result_send.clone().expect("must not be shut down");
        self.running_jobs.insert(
            job_id,
            RunningJob {
                control: control_send,
                job: job.clone(),
            },
        );
        let db_pool = self.db_pool.clone();
        let storage = self.storage.clone();
        tokio::task::spawn(async move {
            let result = process(db_pool, storage, file_id, job, control_recv).await;
            let _ = result_send.send((job_id, result)).await;
        });
    }

    fn new_job_id(&mut self) -> JobId {
        self.next_job_id += 1;
        JobId(self.next_job_id)
    }
}

async fn process(
    db_pool: DbPool,
    storage: Storage,
    file_id: FileId,
    job: ImageJob,
    mut control_recv: JobControlRecv,
) -> Result<Result<()>, JobError> {
    let mut conn = db_pool.get().await?;
    match job {
        ImageJob::CreateThumbnail {
            thumbnail_types,
            formats,
        } => {
            tracing::trace!(?file_id, ?thumbnail_types, ?formats, "create thumbnail");
            let (process_control_send, mut process_control_recv) = mpsc::channel(1);
            let result_fut = generate_asset_thumbnail(
                &mut conn,
                &storage,
                file_id,
                &thumbnail_types,
                &formats,
                &mut process_control_recv,
            );
            run_process_loop(result_fut, &mut control_recv, process_control_send).await??;
        }
        ImageJob::CreateThumbhash => {
            tracing::trace!(?file_id, "create thumbhash");
            generate_thumbhash(&mut conn, &storage, file_id)
                .await
                .wrap_err("error generating asset thumbhash")?;
        }
        ImageJob::ConvertImage(convert_op) => {
            tracing::trace!(?file_id, ?convert_op, "converting image");
            convert_image(&mut conn, &storage, convert_op)
                .await
                .wrap_err("error converting image")?;
        }
    }
    Ok(Ok(()))
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
            outputs.push((storage.local_path(file_key), target));
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
            storage.local_path(&thumbnail_key)
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
    let file_id = op.file_id;
    let (file, asset_path) = interact!(conn, move |conn| {
        // FIXME (low) unnecessarily querying same row twice
        let file = repository::asset::get_asset_file(conn, file_id)?;
        let asset_path = repository::asset::get_asset_path_on_disk(conn, file_id)?;
        Ok((file, asset_path))
    })
    .await??;

    let out_path = storage.local_path(&op.output_file_key);
    let input_path = asset_path.path_on_disk();
    let target = op.target.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let out_path_copy = out_path.clone();
    rayon::spawn(move || {
        let res = processing::image::convert_image(&input_path, &out_path_copy, &target);
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
    let file_size = tokio::fs::metadata(&out_path)
        .await
        .wrap_err_with(|| format!("error reading file metadata for {out_path}"))?
        .size();

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
