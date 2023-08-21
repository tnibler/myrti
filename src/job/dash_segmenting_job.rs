use eyre::{bail, Context, Report, Result};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, instrument, Instrument};

use crate::{
    catalog::encoding_target::EncodingTarget,
    core::{
        job::{Job, JobHandle, JobProgress, JobResultType},
        DataDirManager,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetId, AssetType, AudioRepresentation, AudioRepresentationId, Video, VideoAsset,
        VideoRepresentation, VideoRepresentationId,
    },
    processing::video::{
        dash_package::{shaka_package, RepresentationInput, RepresentationType},
        probe_video,
    },
};

#[derive(Debug, Clone)]
pub enum VideoProcessingTask {
    DashPackageOnly {
        asset_id: AssetId,
    },
    // transcode, gather all representations and package them together
    TranscodeAndPackage {
        asset_id: AssetId,
        encoding_target: EncodingTarget,
    },
}

#[derive(Debug, Clone)]
pub struct DashSegmentingJobParams {
    pub tasks: Vec<VideoProcessingTask>,
}

pub struct DashSegmentingJob {
    params: DashSegmentingJobParams,
    data_dir_manager: Arc<DataDirManager>,
    pool: DbPool,
}

#[derive(Debug)]
pub struct DashSegmentingJobResult {
    pub completed: Vec<AssetId>,
    pub failed: Vec<(AssetId, Report)>,
}

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

    #[instrument(name = "DashSegmentingJob", skip(self, status_tx, cancel))]
    async fn run(
        self,
        status_tx: mpsc::Sender<JobProgress>,
        cancel: CancellationToken,
    ) -> DashSegmentingJobResult {
        let mut failed: Vec<(AssetId, Report)> = vec![];
        let mut completed: Vec<AssetId> = vec![];
        for task in self.params.tasks.iter() {
            if cancel.is_cancelled() {
                break;
            }
            match task {
                VideoProcessingTask::DashPackageOnly { asset_id } => {
                    let _ = status_tx
                        .send(JobProgress {
                            percent: None,
                            description: format!("Processing {}", asset_id.0),
                        })
                        .await;
                    match self.dash_package_asset(*asset_id).in_current_span().await {
                        Ok(()) => completed.push(*asset_id),
                        Err(e) => failed.push((*asset_id, e.wrap_err("error packaging video"))),
                    }
                }
                _ => todo!(),
            }
        }
        DashSegmentingJobResult { completed, failed }
    }

    #[instrument(skip(self))]
    async fn dash_package_asset(&self, asset_id: AssetId) -> Result<()> {
        let asset = repository::asset::get_asset(&self.pool, asset_id)
            .in_current_span()
            .await?;
        let asset: VideoAsset = asset.try_into()?;
        let asset_path = repository::asset::get_asset_path_on_disk(&self.pool, asset_id)
            .await
            .unwrap();
        let output_dir = match asset.video.dash_resource_dir {
            Some(p) => p,
            None => self
                .data_dir_manager
                .new_dash_dir(format!("{}", asset_id.0).as_str())
                .await
                .wrap_err("could not create DASH output directory")?,
        };
        // Final directory structure:
        // dash/:id
        //       | audio.mp4
        //       | h264
        //          | 1920x1080.mp4
        let video_out_dir = output_dir.join(format!("{}", asset.video.codec_name));
        let video_out_filename = PathBuf::from("original.mp4");
        let video_out_path = video_out_dir.join(video_out_filename);
        let audio_out_dir = &output_dir;
        let audio_out_filename = PathBuf::from("audio.mp4");
        let audio_out_path = audio_out_dir.join(audio_out_filename);
        let mpd_out_path = output_dir.join("stream.mpd");
        tokio::fs::create_dir_all(&video_out_dir)
            .await
            .wrap_err("could not create video output directory")?;
        tokio::fs::create_dir_all(&audio_out_dir)
            .await
            .wrap_err("could not create audio output directory")?;
        let reprs = [
            RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Video,
                out_path: video_out_path.clone(),
            },
            RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Audio,
                out_path: audio_out_path.clone(),
            },
        ];
        shaka_package(&reprs, &mpd_out_path)
            .in_current_span()
            .await
            .wrap_err("error packaging video for DASH")?;
        // little annoying to call ffprobe again here
        // if one day we store all the metadata in db we can save the call
        let probe_result = probe_video(&asset_path.path_on_disk()).await?;
        let video_representation = VideoRepresentation {
            id: VideoRepresentationId(0),
            asset_id,
            codec_name: asset.video.codec_name.clone(),
            width: asset.base.size.width,
            height: asset.base.size.height,
            bitrate: probe_result.bitrate,
            path: video_out_path,
        };
        let audio_representation = AudioRepresentation {
            id: AudioRepresentationId(0),
            asset_id,
            path: audio_out_path,
        };
        let mut tx = self
            .pool
            .begin()
            .await
            .wrap_err("could not begin db transaction")?;
        let _ =
            repository::representation::insert_video_representation(&mut tx, video_representation)
                .await?;
        let _ =
            repository::representation::insert_audio_representation(&mut tx, audio_representation)
                .await?;
        // TODO don't do this every time
        let updated_asset = VideoAsset {
            video: Video {
                dash_resource_dir: Some(output_dir),
                ..asset.video
            },
            base: asset.base,
        };
        repository::asset::update_asset(&mut tx, &updated_asset.into()).await?;
        tx.commit().await?;
        Ok(())
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
