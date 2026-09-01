use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::os::unix::fs::MetadataExt;

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
use crate::processing;
use crate::processing::image::thumbnail::{
    ThumbnailParams, ThumbnailResult, generate_thumbnail, generate_video_thumbnail,
};
use crate::processing::process_control::ProcessControl;
use crate::{
    catalog::storage_key, core::storage::Storage,
    processing::process_control::ProcessControlReceiver,
};

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Job was cancelled")]
    Cancelled,
    #[error("Job failed")]
    Other(#[from] eyre::Report),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(u64);

#[derive(Clone)]
struct RunningJob {
    control: JobControl,
    jobs: Vec<ImageJob>,
}

#[derive(Clone)]
struct JobControl {
    send: mpsc::UnboundedSender<JobControlMsg>,
}

pub struct JobControlRecv {
    pub recv: mpsc::UnboundedReceiver<JobControlMsg>,
}

#[derive(Debug, Clone)]
pub enum JobControlMsg {
    Pause,
    Resume,
    Cancel,
}

fn new_job_control() -> (JobControl, JobControlRecv) {
    let (send, recv) = mpsc::unbounded_channel();
    (JobControl { send }, JobControlRecv { recv })
}

#[derive(Clone)]
pub(super) struct ImageJobProcessor {
    max_running: usize,
    max_queued: usize,
    queue: VecDeque<(JobId, FileId, Vec<ImageJob>)>,
    accepting_new: bool,
    // back up to scheduler
    result_send: mpsc::Sender<ImageProcessingMsg>,
    running_jobs: HashMap<JobId, RunningJob>,
    next_job_id: u64,

    db_pool: DbPool,
    storage: Storage,
    config: Config,
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

pub(super) type ImageProcessingMsg = (JobId, Result<Result<()>, JobError>);

impl ImageJobProcessor {
    pub fn new(
        max_running: NonZeroUsize,
        max_queued: NonZeroUsize,
        result_send: mpsc::Sender<ImageProcessingMsg>,
        db_pool: DbPool,
        storage: Storage,
        config: Config,
    ) -> Self {
        Self {
            max_running: max_running.get(),
            max_queued: max_queued.get(),
            queue: Default::default(),
            accepting_new: true,
            result_send,
            running_jobs: Default::default(),
            next_job_id: 0,
            db_pool,
            storage,
            config,
        }
    }

    pub fn enqueue_job(&mut self, file_id: FileId, job: ImageJob) -> Result<JobId, ImageJob> {
        if let Some((job_id, _file_id, jobs)) = self
            .queue
            .iter_mut()
            .rfind(|(_job_id, queued_file_id, _jobs)| *queued_file_id == file_id)
        {
            jobs.push(job);
            Ok(*job_id)
        } else if self.accepting_new && self.running_jobs.len() < self.max_running {
            let job_id = self.new_job_id();
            self.start_job(job_id, file_id, vec![job]);
            Ok(job_id)
        } else if self.queue.len() < self.max_queued {
            let job_id = self.new_job_id();
            self.queue.push_back((job_id, file_id, vec![job]));
            Ok(job_id)
        } else {
            Err(job)
        }
    }

    pub fn try_dequeue(&mut self) {
        let mut dequeued_count = 0;
        while self.accepting_new && self.running_jobs.len() < self.max_running {
            if let Some((job_id, file_id, jobs)) = self.queue.pop_front() {
                dequeued_count += 1;
                self.start_job(job_id, file_id, jobs);
            } else {
                break;
            }
        }
        let remaining_queued = self.queued();
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
        tracing::debug!("cancelling all");
        self.accepting_new = false;
        for job in self.running_jobs.values() {
            let _ = job.control.send.send(JobControlMsg::Cancel);
        }
    }

    pub fn on_job_finished(&mut self, job_id: JobId) {
        if self.running_jobs.remove(&job_id).is_none() {
            tracing::error!(?job_id, "tried removing non existing job from running_jobs");
        }
        self.try_dequeue();
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn start_job(&mut self, job_id: JobId, file_id: FileId, jobs: Vec<ImageJob>) {
        tracing::trace!(?job_id, ?file_id, "starting job");
        assert!(self.running_jobs.len() < self.max_running);
        let (control_send, control_recv) = new_job_control();
        let result_send = self.result_send.clone();
        self.running_jobs.insert(
            job_id,
            RunningJob {
                control: control_send,
                jobs: jobs.clone(),
            },
        );
        let db_pool = self.db_pool.clone();
        let storage = self.storage.clone();
        let config = self.config.clone();
        tokio::task::spawn(async move {
            let result = process(db_pool, storage, config, file_id, jobs, control_recv).await;
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
    config: Config,
    file_id: FileId,
    jobs: Vec<ImageJob>,
    mut control_recv: JobControlRecv,
) -> Result<Result<()>, JobError> {
    let mut conn = db_pool.get().await?;
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
            ImageJob::PackageVideo(package_op) => {
                tracing::trace!(?file_id, ?package_op, "packaging video");
                let (process_control_send, process_control_recv) = tokio::sync::mpsc::channel(1);
                let result_fut = do_package_video(
                    &db_pool,
                    &storage,
                    package_op.clone(),
                    config.bin_paths.as_ref(),
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

async fn run_process_loop<T>(
    fut: impl Future<Output = T>,
    ctl_recv: &mut JobControlRecv,
    process_control_send: mpsc::Sender<ProcessControl>,
) -> Result<T, JobError> {
    let mut was_cancelled = false;
    let mut fut = std::pin::pin!(fut);
    loop {
        tokio::select! {
            result = &mut fut => {
                if was_cancelled {
                    return Err(JobError::Cancelled)
                } else {
                    return Ok(result);
                }
            }
            Some(msg) = ctl_recv.recv.recv() => {
                if was_cancelled {
                    continue;
                }
                let (process_control, will_cancel) = match msg {
                    JobControlMsg::Pause => (ProcessControl::Suspend, false),
                    JobControlMsg::Resume => (ProcessControl::Resume, false),
                    JobControlMsg::Cancel => (ProcessControl::Quit, true),
                };
                match process_control_send.send(process_control).await {
                    Ok(_) => {
                        was_cancelled = will_cancel;
                    }
                    Err(err) => {
                        return Err(JobError::Other(eyre::Report::from(err).wrap_err("error sending process control message")));
                    }
                };
            }
        }
    }
}
