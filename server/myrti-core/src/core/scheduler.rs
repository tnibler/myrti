use std::{str::FromStr, time::Duration};

use camino::Utf8Path as Path;
use eyre::{Context, Result, eyre};
use futures::{TryStreamExt, stream::FuturesUnordered};
use strum::EnumCount;
use tokio::sync::{mpsc, oneshot};
use tracing::instrument;

use myrti_data::db::{DbPool, PooledDbConn};
use myrti_data::model::{AssetId, AssetRootDirId, AssetSpe, FileId};
use myrti_data::{interact, repository};

use crate::core::image_processor::{
    ImageJob, ImageJobProcessor, ImageProcessingMsg, ImageProcessor,
};
use crate::core::queue_executor::{BatchQueueExecutor, JobError, JobId};
use crate::{
    actor::indexing::{IndexingActorHandle, MsgFromIndexing},
    catalog::{
        operation::{create_thumbnail::CreateAssetThumbnail, package_video::PackageVideo},
        rules,
    },
    config::{BinPaths, Config},
};

use super::storage::Storage;

#[derive(Debug)]
pub enum SchedulerMessage {
    Startup,
    Timer,
    UserRequest(UserRequest),
    PauseAllProcessing,
    ResumeAllProcessing,
    PauseVideoPackaging,
    ResumeVideoPackaging,
    Shutdown,
}

#[derive(Debug)]
pub enum UserRequest {
    ReindexAssetRoot(AssetRootDirId),
    RegenerateThumbnails(Option<Vec<FileId>>),
    DisableGhiStreaming(Vec<FileId>),
}

#[derive(Debug, Clone)]
pub struct SchedulerHandle {
    pub send: mpsc::Sender<SchedulerMessage>,
}

#[derive(Debug, Copy, Clone, strum::EnumCount)]
#[repr(usize)]
enum Actors {
    Indexing,
    VideoPackaging,
}

#[derive(Debug, Default)]
struct ActorState {
    has_dropped_msgs: bool,
}

struct Scheduler {
    db_pool: DbPool,
    storage: Storage,
    config: Config,

    waiting_for_shutdown: bool,
    did_shutdown_send: Option<oneshot::Sender<()>>,
    actor_did_shutdown_recvs: Option<Vec<oneshot::Receiver<()>>>,

    actor_states: [ActorState; Actors::COUNT],

    indexing_actor: IndexingActorHandle,
    image_proc: ImageProcessor,
    dropped_image_jobs: bool,
}

impl SchedulerHandle {
    pub fn new(
        db_pool: DbPool,
        storage: Storage,
        config: Config,
        did_shutdown_send: oneshot::Sender<()>,
    ) -> Self {
        let (indexing_did_shutdown_send, indexing_did_shutdown_recv) = oneshot::channel::<()>();
        let (from_indexing_send, from_indexing_recv) = mpsc::unbounded_channel();
        let indexing_actor = IndexingActorHandle::new(
            db_pool.clone(),
            config.clone(),
            indexing_did_shutdown_send,
            from_indexing_send,
        );

        let (from_thumbnail_send, from_thumbnail_recv) = mpsc::channel(100);

        let thumbnail_proc = ImageJobProcessor {
            db_pool: db_pool.clone(),
            storage: storage.clone(),
            config: config.clone(),
        };

        let (send, recv) = mpsc::channel(1000);
        let sched = Scheduler {
            db_pool,
            storage,
            config,
            waiting_for_shutdown: false,
            did_shutdown_send: Some(did_shutdown_send),
            actor_did_shutdown_recvs: Some(vec![indexing_did_shutdown_recv]),
            actor_states: Default::default(),
            indexing_actor: indexing_actor.clone(),
            image_proc: BatchQueueExecutor::new(
                thumbnail_proc,
                4.try_into().unwrap(),
                1000.try_into().unwrap(),
                from_thumbnail_send,
            ),
            dropped_image_jobs: false,
        };
        tokio::spawn(run_scheduler(
            sched,
            recv,
            from_indexing_recv,
            from_thumbnail_recv,
        ));
        Self { send }
    }
}

async fn run_scheduler(
    mut sched: Scheduler,
    mut recv: mpsc::Receiver<SchedulerMessage>,
    mut indexing_recv: mpsc::UnboundedReceiver<MsgFromIndexing>,
    mut thumbnail_recv: mpsc::Receiver<ImageProcessingMsg>,
) {
    let mut have_written_to_disk = true;
    let mut reindex_interval = {
        let mut int = tokio::time::interval(Duration::from_mins(60));
        int.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        int
    };
    let mut check_disk_interval = {
        let mut int = tokio::time::interval(Duration::from_mins(5));
        int.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        int
    };
    loop {
        tokio::select! {
            _ = reindex_interval.tick(), if !sched.waiting_for_shutdown => {
                if let Err(err) = reindex_all(&sched.db_pool, &sched.indexing_actor).await {
                    tracing::error!(?err, "Error reindexing asset roots");
                }
            }
            _ = check_disk_interval.tick(), if !sched.waiting_for_shutdown && have_written_to_disk => {
                have_written_to_disk = false;
                match is_disk_almost_full(&sched.config.data_dir.path).await {
                    Ok(false) => {}
                    Ok(true) => {
                        tracing::warn!("Disk almost full, pausing processing");
                        sched.handle_message(SchedulerMessage::PauseAllProcessing).await;
                    }
                    Err(err) => {
                        tracing::warn!(?err, "error checking disk usage");
                    }
                }
            }
            Some(msg) = recv.recv() => {
                sched.handle_message(msg).await;
            }
            Some(indexing_msg) = indexing_recv.recv() => {
                have_written_to_disk = true;
                if let Err(err) = sched.on_indexing_msg(indexing_msg).await {
                    tracing::error!(?err, "error in scheduler");
                }
            }
            Some(thumbnail_msg) = thumbnail_recv.recv() => {
                have_written_to_disk = true;
                if let Err(err) = sched.on_image_msg(thumbnail_msg).await {
                    tracing::error!(?err, "error in scheduler");
                }
            }
            else => {
                break;
            }
        }
    }
    tracing::info!("Exiting main loop");
}

impl Scheduler {
    async fn on_indexing_msg(&mut self, msg: MsgFromIndexing) -> Result<()> {
        let actor_state = &mut self.actor_states[Actors::Indexing as usize];
        match msg {
            MsgFromIndexing::ActivityChange {
                running_tasks,
                queued_tasks,
            } => {
                let is_idle = running_tasks == 0 && queued_tasks == 0;
                if is_idle && actor_state.has_dropped_msgs {
                    actor_state.has_dropped_msgs = false;
                    // TODO: what do we do here. reindex all? for other actors we look up in db
                    // what work they can do but this one is different
                }
            }
            MsgFromIndexing::DroppedMessage => {
                actor_state.has_dropped_msgs = true;
            }
            MsgFromIndexing::NewAsset(asset_id) => {
                if let Err(err) = self.on_new_asset_indexed(asset_id).await {
                    tracing::error!(?err, "error in on_new_asset_indexed");
                }
            }
            MsgFromIndexing::IndexingError {
                root_dir_id,
                path,
                report,
            } => {
                tracing::error!(
                    ?root_dir_id,
                    ?path,
                    ?report,
                    "TODO unhandled indexing error"
                );
            }
            MsgFromIndexing::IndexingComplete { root_dir_id } => {
                tracing::debug!(?root_dir_id, "Completed indexing root directory");
            }
            MsgFromIndexing::IndexingCancelled { root_dir_id } => {
                tracing::debug!(?root_dir_id, "Cancelled indexing root directory");
            }
            MsgFromIndexing::FailedToStartIndexing {
                root_dir_id,
                report,
            } => {
                tracing::error!(?root_dir_id, %report, "TODO unhandled failed to start indexing job");
            }
        }
        Ok(())
    }

    async fn on_new_asset_indexed(&mut self, asset_id: AssetId) -> Result<()> {
        let mut conn = self.db_pool.get().await.unwrap();
        let asset = interact!(conn, move |conn| repository::asset::get_asset(
            conn, asset_id
        ))
        .await??;
        let thumbnails_required =
            rules::required_thumbnails_for_asset(&mut conn, asset.base.rep_file_id).await?;
        if !thumbnails_required.thumbnail_types.is_empty()
            && self
                .image_proc
                .enqueue_job(
                    thumbnails_required.file_id,
                    ImageJob::CreateThumbnail {
                        thumbnail_types: thumbnails_required.thumbnail_types,
                        formats: thumbnails_required.formats,
                    },
                )
                .is_err()
        {
            self.dropped_image_jobs = true;
        }
        match &asset.sp {
            AssetSpe::Video(video) => {
                let video_packaging_required = rules::required_video_packaging_for_asset(
                    &mut conn,
                    video.file_id,
                    self.config.bin_paths.as_ref(),
                )
                .await?;
                for vid_pack in video_packaging_required {
                    if self
                        .image_proc
                        .enqueue_job(vid_pack.file_id, ImageJob::PackageVideo(vid_pack))
                        .is_err()
                    {
                        self.dropped_image_jobs = true;
                    }
                }
            }
            AssetSpe::Image(image) => {
                let image_conversion_required =
                    rules::required_image_conversion_for_asset(&mut conn, image.file_id).await?;
                for img_convert in image_conversion_required {
                    if self
                        .image_proc
                        .enqueue_job(img_convert.file_id, ImageJob::ConvertImage(img_convert))
                        .is_err()
                    {
                        self.dropped_image_jobs = true;
                    }
                }

                if self
                    .image_proc
                    .enqueue_job(asset.rep_file.id, ImageJob::CreateThumbhash)
                    .is_err()
                {
                    self.dropped_image_jobs = true;
                }
            }
        }

        Ok(())
    }

    async fn on_image_msg(
        &mut self,
        (job_id, result): (JobId, Result<Result<()>, JobError>),
    ) -> Result<()> {
        match result {
            Err(JobError::Cancelled) => {
                tracing::debug!(?job_id, "image job cancelled");
            }
            Err(JobError::Other(err)) => {
                tracing::warn!(?job_id, ?err, "image job encountered an error");
            }
            Ok(Err(err)) => {
                tracing::warn!(?job_id, ?err, "image job encountered an error");
            }
            Ok(Ok(())) => {
                tracing::trace!(?job_id, "image job completed");
            }
        }
        self.image_proc.on_job_finished(job_id);
        if self.dropped_image_jobs && self.image_proc.queued() == 0 {
            tracing::debug!("job queue empty, enqueueing any dropped jobs");
            let mut conn = self.db_pool.get().await?;
            self.dropped_image_jobs = false;
            self.enqueue_required_image_jobs(&mut conn).await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, msg: SchedulerMessage) {
        if self.waiting_for_shutdown {
            tracing::trace!(?msg, "waiting for shutdown, ignoring");
            return;
        }
        match msg {
            SchedulerMessage::Timer => {}
            SchedulerMessage::UserRequest(user_request) => match user_request {
                UserRequest::ReindexAssetRoot(root_dir_id) => {
                    let _ = self.indexing_actor.msg_index_asset_root(root_dir_id);
                }
                UserRequest::DisableGhiStreaming(file_ids) => {
                    tracing::debug!(?file_ids, "request DisableGhiStreaming");
                    async fn handle_disable_ghi(
                        db_pool: DbPool,
                        bin_paths: Option<&BinPaths>,
                        file_ids: Vec<FileId>,
                    ) -> Result<Vec<PackageVideo>> {
                        let mut conn = db_pool.get().await?;

                        for file_id in file_ids.clone() {
                            interact!(conn, move |conn| {
                                repository::asset::set_asset_ghi_disabled(conn, file_id, true)
                            })
                            .await??;
                        }
                        let required_video_packaging = {
                            let mut required = Vec::default();
                            for file_id in file_ids {
                                required.extend(
                                    rules::required_video_packaging_for_asset(
                                        &mut conn, file_id, bin_paths,
                                    )
                                    .await?,
                                );
                            }
                            required
                        };
                        Ok(required_video_packaging)
                    }

                    match handle_disable_ghi(
                        self.db_pool.clone(),
                        self.config.bin_paths.as_ref(),
                        file_ids,
                    )
                    .await
                    {
                        Ok(tasks) => {
                            for p in tasks {
                                if self
                                    .image_proc
                                    .enqueue_job(p.file_id, ImageJob::PackageVideo(p))
                                    .is_err()
                                {
                                    self.dropped_image_jobs = true;
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(?err, "error setting GHI streaming disabled")
                        }
                    }
                }
                UserRequest::RegenerateThumbnails(file_ids) => {
                    tracing::debug!(?file_ids, "request RegenerateThumbnails");
                    async fn handle_regenerate_thumbnails(
                        db_pool: DbPool,
                        file_ids: Option<Vec<FileId>>,
                    ) -> Result<Vec<CreateAssetThumbnail>> {
                        let mut conn = db_pool.get().await?;

                        let file_ids = interact!(conn, move |conn| {
                            repository::asset::delete_thumbnails_for_file(
                                conn,
                                file_ids.as_deref(),
                            )?;
                            Ok(file_ids)
                        })
                        .await??;

                        let required_thumbnails = if let Some(file_ids) = file_ids {
                            let mut required_thumbnails = Vec::default();
                            for file_id in file_ids {
                                required_thumbnails.push(
                                    rules::required_thumbnails_for_asset(&mut conn, file_id)
                                        .await?,
                                );
                            }
                            required_thumbnails
                        } else {
                            rules::thumbnails_to_create(&mut conn).await?
                        };

                        Ok(required_thumbnails)
                    }

                    match handle_regenerate_thumbnails(self.db_pool.clone(), file_ids).await {
                        Ok(create_thumbs) => {
                            for CreateAssetThumbnail {
                                file_id,
                                formats,
                                thumbnail_types,
                            } in create_thumbs
                            {
                                if self
                                    .image_proc
                                    .enqueue_job(
                                        file_id,
                                        ImageJob::CreateThumbnail {
                                            thumbnail_types,
                                            formats,
                                        },
                                    )
                                    .is_err()
                                {
                                    self.dropped_image_jobs = true;
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(?err, "error regenerating thumbnails")
                        }
                    }
                }
            },
            SchedulerMessage::PauseAllProcessing => {
                self.image_proc.pause_all();
            }
            SchedulerMessage::ResumeAllProcessing => {
                self.image_proc.resume_all();
            }
            SchedulerMessage::PauseVideoPackaging => {
                todo!()
            }
            SchedulerMessage::ResumeVideoPackaging => {
                todo!()
            }
            SchedulerMessage::Shutdown => {
                if !self.waiting_for_shutdown {
                    self.waiting_for_shutdown = true;
                    self.indexing_actor
                        .msg_shutdown()
                        .expect("receiver must be alive");
                    self.image_proc.cancel_all();
                    let did_shutdown_recvs = self
                        .actor_did_shutdown_recvs
                        .take()
                        .expect("must be Some before shutdown called");
                    let did_shutdown_send = self
                        .did_shutdown_send
                        .take()
                        .expect("must be Some before shutdown called");
                    tokio::task::spawn(async move {
                        did_shutdown_recvs
                            .into_iter()
                            .collect::<FuturesUnordered<_>>()
                            .try_collect::<Vec<()>>()
                            .await
                            .expect("TODO senders must be alive");
                        tracing::info!("all actors shutdown");
                        did_shutdown_send.send(()).expect("receiver must be alive");
                    });
                } else {
                    tracing::debug!(
                        "Already waiting for shutdown, received another shutdown message"
                    );
                }
            }
            SchedulerMessage::Startup => {
                self.on_startup().await;
            }
        }
    }

    #[instrument(skip_all)]
    async fn on_startup(&mut self) {
        let mut conn = self
            .db_pool
            .get()
            .await
            .expect("TODO how do we handle errors in scheduler");

        if let Err(err) = interact!(conn, move |conn| {
            repository::timeline::rebuild_timeline_full(conn)
        })
        .await
        .flatten()
        {
            tracing::error!("Error rebuilding timeline:\n{:?}", (err));
        }

        let album_thumbnails_required = rules::album_thumbnails_to_create(&mut conn)
            .await
            .expect("TODO");

        if let Err(err) = self.enqueue_required_image_jobs(&mut conn).await {
            tracing::error!(?err, "Error enqueuing image processing jobs");
        }
        // for album_thumb in album_thumbnails_required {
        //     let _ = self.thumbnail_actor.msg_create_album_thumbnail(album_thumb);
        // }

        if let Err(err) = reindex_all(&self.db_pool, &self.indexing_actor).await {
            tracing::error!(?err, "Error reindexing asset roots");
        }
    }

    async fn enqueue_required_image_jobs(&mut self, conn: &mut PooledDbConn) -> Result<()> {
        let thumbnails_required = rules::thumbnails_to_create(conn).await?;
        let thumbhash_missing = rules::files_missing_thumbhash(conn).await?;
        for file_id in thumbhash_missing {
            if self
                .image_proc
                .enqueue_job(file_id, ImageJob::CreateThumbhash)
                .is_err()
            {
                self.dropped_image_jobs = true;
            }
        }

        for CreateAssetThumbnail {
            file_id,
            formats,
            thumbnail_types,
        } in thumbnails_required
        {
            if self
                .image_proc
                .enqueue_job(
                    file_id,
                    ImageJob::CreateThumbnail {
                        thumbnail_types,
                        formats,
                    },
                )
                .is_err()
            {
                self.dropped_image_jobs = true;
            }
        }

        let image_conversion_required = rules::image_conversion_due(conn).await?;
        for img_convert in image_conversion_required {
            if self
                .image_proc
                .enqueue_job(img_convert.file_id, ImageJob::ConvertImage(img_convert))
                .is_err()
            {
                self.dropped_image_jobs = true;
            }
        }

        let video_packaging_required = rules::video_packaging_due(conn).await?;
        for v in video_packaging_required {
            if self
                .image_proc
                .enqueue_job(v.file_id, ImageJob::PackageVideo(v))
                .is_err()
            {
                self.dropped_image_jobs = true;
            }
        }
        Ok(())
    }
}

async fn reindex_all(db_pool: &DbPool, indexing_actor: &IndexingActorHandle) -> Result<()> {
    tracing::debug!("reindexing all");
    let conn = db_pool.get().await?;
    let res = interact!(conn, move |conn| {
        repository::asset_root_dir::get_asset_roots(conn)
    })
    .await?
    .context("error querying asset roots")?;
    for root_dir in res {
        let _ = indexing_actor.msg_index_asset_root(root_dir.id);
    }
    Ok(())
}

async fn is_disk_almost_full(path: &Path) -> Result<bool> {
    let output = tokio::process::Command::new("df")
        .arg(path)
        .args([
            "--output=source,fstype,size,used,avail",
            "-B",
            DF_BLOCK_SIZE,
        ])
        .output()
        .await
        .wrap_err("error running df")?;
    if !output.status.success() {
        return Err(eyre!(
            "df exited with nonzero status code. stderr: {}",
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    let output = str::from_utf8(&output.stdout).wrap_err("df output is not valid utf-8")?;
    let disk_usage =
        parse_df_output(output).ok_or_else(|| eyre!("unexpected df output format:\n{}", output))?;
    tracing::info!(?disk_usage, "checking disk usage");
    Ok(disk_usage.avail_mb < 5000)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DfOutput {
    fs_source: String,
    fs_type: String,
    size_mb: u64,
    used_mb: u64,
    avail_mb: u64,
}

const DF_BLOCK_SIZE: &str = "MB"; // 10^6 Bytes
fn parse_df_output(output: &str) -> Option<DfOutput> {
    let (_header, values) = output.split_once('\n')?;
    let mut columns = values.split_ascii_whitespace();
    let fs_source = columns.next()?.to_owned();
    let fs_type = columns.next()?.to_owned();
    let size_mb = u64::from_str(columns.next()?.strip_suffix(DF_BLOCK_SIZE)?).ok()?;
    let used_mb = u64::from_str(columns.next()?.strip_suffix(DF_BLOCK_SIZE)?).ok()?;
    let avail_mb = u64::from_str(columns.next()?.strip_suffix(DF_BLOCK_SIZE)?).ok()?;
    Some(DfOutput {
        fs_source,
        fs_type,
        size_mb,
        used_mb,
        avail_mb,
    })
}

#[test]
fn test_parse_df_output() {
    let output = r#"Filesystem            Type 1MB-blocks      Used    Avail
/dev/mapper/cryptroot ext4  1916555MB 1302472MB 516652MB"#;
    assert_eq!(
        parse_df_output(output),
        Some(DfOutput {
            fs_source: "/dev/mapper/cryptroot".to_owned(),
            fs_type: "ext4".to_owned(),
            size_mb: 1916555,
            used_mb: 1302472,
            avail_mb: 516652
        })
    );
}
