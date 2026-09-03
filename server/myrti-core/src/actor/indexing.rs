use std::collections::{HashSet, VecDeque};

use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Result, eyre};
use globset::GlobSet;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

use myrti_data::db::DbPool;
use myrti_data::model::{AssetId, AssetRootDir, AssetRootDirId};
use myrti_data::{interact, repository};

use crate::{config, processing::indexing::try_index_file};

#[derive(Debug)]
pub enum MsgFromIndexing {
    ActivityChange {
        running_tasks: usize,
        queued_tasks: usize,
    },
    DroppedMessage,
    NewAsset(AssetId),
    IndexingError {
        root_dir_id: AssetRootDirId,
        path: Option<PathBuf>,
        report: eyre::Report,
    },
    IndexingComplete {
        root_dir_id: AssetRootDirId,
    },
    FailedToStartIndexing {
        root_dir_id: AssetRootDirId,
        report: eyre::Report,
    },
    IndexingCancelled {
        root_dir_id: AssetRootDirId,
    },
}

#[derive(Debug, Clone)]
enum MsgToIndexing {
    Pause,
    Resume,
    Shutdown,
    DoTask(DoTaskMsg),
}

#[derive(Debug, Clone)]
enum DoTaskMsg {
    IndexAssetRootDir { root_dir_id: AssetRootDirId },
}

#[derive(Clone)]
pub struct IndexingActorHandle {
    send: mpsc::UnboundedSender<MsgToIndexing>,
}

impl IndexingActorHandle {
    pub fn new(
        db_pool: DbPool,
        config: config::Config,
        send_from_us: mpsc::UnboundedSender<MsgFromIndexing>,
    ) -> Self {
        let (send, recv) = mpsc::unbounded_channel();
        let (subtask_send, subtask_recv) = mpsc::unbounded_channel();
        let actor = IndexingActor {
            db_pool,
            config,
            send_from_us,
            subtask_send,
            subtask_recv,
            running_tasks: Default::default(),
            cancel: CancellationToken::new(),
        };
        tokio::spawn(run_indexing_actor(recv, actor));
        Self { send }
    }

    pub fn msg_index_asset_root(&self, root_dir_id: AssetRootDirId) -> Result<()> {
        self.send
            .send(MsgToIndexing::DoTask(DoTaskMsg::IndexAssetRootDir {
                root_dir_id,
            }))?;
        Ok(())
    }

    pub fn msg_pause_all(&self) -> Result<()> {
        self.send.send(MsgToIndexing::Pause)?;
        Ok(())
    }

    pub fn msg_resume_all(&self) -> Result<()> {
        self.send.send(MsgToIndexing::Resume)?;
        Ok(())
    }

    pub fn msg_shutdown(&self) -> Result<()> {
        self.send.send(MsgToIndexing::Shutdown)?;
        Ok(())
    }
}

struct IndexingActor {
    pub db_pool: DbPool,
    pub config: config::Config,
    pub send_from_us: mpsc::UnboundedSender<MsgFromIndexing>,
    subtask_recv: mpsc::UnboundedReceiver<(AssetRootDirId, MsgFromIndexing)>,
    subtask_send: mpsc::UnboundedSender<(AssetRootDirId, MsgFromIndexing)>,
    running_tasks: HashSet<AssetRootDirId>,
    cancel: CancellationToken,
}

const MAX_TASKS: usize = 4;
const MAX_QUEUE_SIZE: usize = 10;

async fn run_indexing_actor(
    mut recv: mpsc::UnboundedReceiver<MsgToIndexing>,
    mut actor: IndexingActor,
) {
    let mut is_running = true;
    let mut queue: VecDeque<DoTaskMsg> = Default::default();
    loop {
        tokio::select! {
            Some((asset_root_id, msg)) = actor.subtask_recv.recv() => {
                tracing::trace!(target="indexing", ?msg, "message from subtask");
                match &msg {
                    MsgFromIndexing::FailedToStartIndexing { root_dir_id, .. }
                    | MsgFromIndexing::IndexingComplete { root_dir_id }
                    | MsgFromIndexing::IndexingCancelled { root_dir_id } => {
                        debug_assert_eq!(*root_dir_id, asset_root_id);
                        let was_running = actor.running_tasks.remove(&asset_root_id);
                        debug_assert!(was_running);
                        actor.send_from_us.send(MsgFromIndexing::ActivityChange {
                            running_tasks: actor.running_tasks.len(),
                            queued_tasks: queue.len()
                        }).expect("receiver must be alive");
                        if actor.cancel.is_cancelled() && actor.running_tasks.is_empty() {
                            tracing::debug!(target="indexing", "last indexing child task cancelled, shutting down");
                            debug_assert!(!is_running);
                            return;
                        }
                    },
                    MsgFromIndexing::IndexingError {..} => {},
                    MsgFromIndexing::DroppedMessage => {},
                    MsgFromIndexing::NewAsset(_) => {},
                    MsgFromIndexing::ActivityChange { .. } => panic!("not a message subtasks should send"),
                }
                // Forward to supervising task/scheduler
                let _ = actor.send_from_us.send(msg);
            },
            Some(msg) = recv.recv() => {
                tracing::trace!(target="indexing", ?msg, "message to indexing actor");
                match msg {
                    MsgToIndexing::Shutdown => {
                        is_running = false;
                        actor.cancel.cancel();
                        if actor.running_tasks.is_empty() {
                            tracing::debug!(target="indexing", "no indexing child tasks running, shutting down");
                            return;
                        }
                    }
                    MsgToIndexing::Pause => {
                        is_running = false;
                        // TODO: pause currently running indexing jobs
                    }
                    MsgToIndexing::Resume => {
                        is_running = true;
                        // TODO: unpause currently running indexing jobs
                    }
                    MsgToIndexing::DoTask(task) => {
                        if is_running && actor.running_tasks.len() < MAX_TASKS {
                            let _ = actor.send_from_us.send(MsgFromIndexing::ActivityChange {
                                running_tasks: actor.running_tasks.len(),
                                queued_tasks: queue.len()
                            });
                            actor.process_message(task).await;
                        } else if queue.len() < MAX_QUEUE_SIZE {
                            queue.push_back(task);
                            let _ = actor.send_from_us.send(MsgFromIndexing::ActivityChange {
                                running_tasks: actor.running_tasks.len(),
                                queued_tasks: queue.len()
                            });
                        } else {
                            let _ = actor.send_from_us.send(MsgFromIndexing::DroppedMessage);
                        }
                    }
                }
            }
            else => {
                break;
            }
        }
    }
}

impl IndexingActor {
    async fn process_message(&mut self, msg: DoTaskMsg) {
        match msg {
            msg if self.cancel.is_cancelled() => {
                tracing::debug!(?msg, "actor shutting down, ignoring message");
            }
            DoTaskMsg::IndexAssetRootDir { root_dir_id } => {
                if self.running_tasks.contains(&root_dir_id) {
                    tracing::trace!(
                        ?root_dir_id,
                        "Indexing task already running, not starting another"
                    );
                    return;
                }

                let send_copy = self.subtask_send.clone();
                let start_result = handle_indexing_message(
                    self.db_pool.clone(),
                    send_copy,
                    &self.config,
                    root_dir_id,
                    self.cancel.child_token(),
                )
                .await;

                if let Err(report) = start_result {
                    let _ = self
                        .send_from_us
                        .send(MsgFromIndexing::FailedToStartIndexing {
                            root_dir_id,
                            report: report.wrap_err("Error starting indexing job"),
                        });
                } else {
                    self.running_tasks.insert(root_dir_id);
                }
            }
        }
    }
}

async fn handle_indexing_message(
    db_pool: DbPool,
    send_result: mpsc::UnboundedSender<(AssetRootDirId, MsgFromIndexing)>,
    config: &config::Config,
    root_dir_id: AssetRootDirId,
    cancel: CancellationToken,
) -> Result<()> {
    let conn = db_pool.get().await?;
    let asset_root = interact!(conn, move |conn| {
        repository::asset_root_dir::get_asset_root(conn, root_dir_id)
    })
    .await?
    .wrap_err("Error getting AssetRootDir from db")?;
    let bin_paths = config.bin_paths.clone();
    let dir_config = config
        .asset_dirs
        .iter()
        .find(|dir| dir.path == asset_root.path);
    let exclude_globs = dir_config.iter().flat_map(|dir| dir.exclude_globs.iter());
    // let data_dir_globs = config.da
    let exclude_set = GlobSet::new(exclude_globs)?;
    tokio::spawn(async move {
        index_asset_root(
            db_pool,
            send_result,
            bin_paths,
            asset_root,
            exclude_set,
            cancel,
        )
        .await;
    });
    Ok(())
}

async fn index_asset_root(
    pool: DbPool,
    send_result: mpsc::UnboundedSender<(AssetRootDirId, MsgFromIndexing)>,
    bin_paths: Option<config::BinPaths>,
    asset_root: AssetRootDir,
    exclude_set: GlobSet,
    cancel: CancellationToken,
) {
    tracing::info!(path=%asset_root.path, "Start indexing");
    // TODO WalkDir is synchronous
    let mut new_asset_count = 0;
    let mut stack: Vec<(PathBuf, i32)> = Default::default();
    for entry in WalkDir::new(asset_root.path.as_path())
        .follow_links(true)
        .into_iter()
        .filter_entry(|ent| !exclude_set.is_match(ent.path()))
    {
        if cancel.is_cancelled() {
            send_result
                .send((
                    asset_root.id,
                    MsgFromIndexing::IndexingCancelled {
                        root_dir_id: asset_root.id,
                    },
                ))
                .expect("receiver must be alive");
            return;
        }
        match entry {
            Ok(entry) if entry.file_type().is_file() => {
                let utf8_path = camino::Utf8Path::from_path(entry.path());
                if let Some(path) = utf8_path {
                    let indexing_res =
                        try_index_file(path, &asset_root, &pool, bin_paths.as_ref()).await;
                    let msg = match indexing_res {
                        Ok(None) => {
                            continue;
                        }
                        Ok(Some(asset_id)) => {
                            new_asset_count += 1;
                            stack
                                .last_mut()
                                .expect("directory must be yielded before files inside id")
                                .1 += 1;
                            MsgFromIndexing::NewAsset(asset_id)
                        }
                        Err(report) => MsgFromIndexing::IndexingError {
                            root_dir_id: asset_root.id,
                            path: Some(path.to_owned()),
                            report,
                        },
                    };
                    send_result
                        .send((asset_root.id, msg))
                        .expect("receiver must be alive");
                }
            }
            Ok(dir) => {
                let path = match PathBuf::try_from(dir.into_path()) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                assert!(path.is_dir());
                if let Some((completed_dir, new_count)) =
                    stack.pop_if(|(top, _)| !path.starts_with(top))
                {
                    // assert_eq!(
                    //     path.parent(),
                    //     stack
                    //         .last()
                    //         .and_then(|(top, _)| top.parent().map(|p| p.as_path()))
                    // );
                    // assert!(
                    //
                    //     stack
                    //         .last()
                    //         .is_none_or(|(top, _)| path.parent() == top.parent()),
                    //     "stack: {stack:?}, path: {path:?}"
                    // );
                    tracing::trace!(?completed_dir, "Completed indexing directory");
                    if new_count > 0 {
                        #[allow(clippy::redundant_closure_call)]
                        if let Err(err) = (async || {
                            let conn = pool.get().await?;
                            interact!(conn, move |conn| {
                                repository::asset::merge_image_assets(conn)
                                    .wrap_err("error trying to merge assets")?;
                                repository::asset::detect_image_sequences(conn)
                                    .wrap_err("error trying to detect image sequences")?;
                                // repository::timeline::update_timeline_dirty(conn)
                                //     .wrap_err("error updating timeline")?;
                                repository::timeline::rebuild_timeline_full(conn)
                                    .wrap_err("error rebuilding timeline")?;
                                Ok(())
                            })
                            .await??;
                            Ok::<_, eyre::Error>(())
                        })()
                        .await
                        {
                            tracing::error!("{:?}", err);
                        }
                    }
                } else {
                    stack.push((path, 0));
                }
            }
            Err(err) => {
                tracing::warn!(target = "indexing", ?err, "error while listing directory");
                send_result
                    .send((
                        asset_root.id,
                        MsgFromIndexing::IndexingError {
                            root_dir_id: asset_root.id,
                            path: err.path().map(|p| {
                                p.to_owned()
                                    .try_into()
                                    .expect("only UTF-8 paths are supported")
                            }),
                            report: eyre!("error while listing directory: {}", err),
                        },
                    ))
                    .expect("receiver must be alive");
            }
        }
    }
    send_result
        .send((
            asset_root.id,
            MsgFromIndexing::IndexingComplete {
                root_dir_id: asset_root.id,
            },
        ))
        .expect("receiver must be alive");
    #[allow(clippy::redundant_closure_call)]
    if let Err(err) = (async || {
        let conn = pool.get().await?;
        interact!(conn, move |conn| {
            repository::asset::merge_image_assets(conn).wrap_err("error trying to merge assets")?;
            repository::asset::detect_image_sequences(conn)
                .wrap_err("error trying to detect image sequences")?;
            // repository::timeline::update_timeline_dirty(conn)
            //     .wrap_err("error updating timeline")?;
            repository::timeline::rebuild_timeline_full(conn)
                .wrap_err("error rebuilding timeline")?;
            Ok(())
        })
        .await??;
        Ok::<_, eyre::Error>(())
    })()
    .await
    {
        tracing::error!("{:?}", err);
    }
    tracing::info!(path=%asset_root.path, new_assets=new_asset_count, "Finished indexing");
}
