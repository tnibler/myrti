use std::collections::{HashSet, VecDeque};

use camino::Utf8PathBuf as PathBuf;
use eyre::{eyre, Context, Result};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use walkdir::WalkDir;

use crate::{
    config, interact,
    model::{
        repository::{self, db::DbPool},
        AssetId, AssetRootDir, AssetRootDirId,
    },
    processing::indexing::index_file,
};

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
        did_shutdown_send: oneshot::Sender<()>,
        send_from_us: mpsc::UnboundedSender<MsgFromIndexing>,
    ) -> Self {
        let (send, recv) = mpsc::unbounded_channel();
        let (subtask_send, subtask_recv) = mpsc::unbounded_channel();
        let actor = IndexingActor {
            db_pool,
            config,
            did_shutdown_send: Some(did_shutdown_send),
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
    did_shutdown_send: Option<oneshot::Sender<()>>,
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
                tracing::debug!(?msg);
                match &msg {
                    MsgFromIndexing::FailedToStartIndexing { root_dir_id, .. }
                    | MsgFromIndexing::IndexingComplete { root_dir_id }
                    | MsgFromIndexing::IndexingCancelled { root_dir_id } => {
                        debug_assert_eq!(*root_dir_id, asset_root_id);
                        let was_running = actor.running_tasks.remove(&asset_root_id);
                        debug_assert!(was_running);
                        let _ = actor.send_from_us.send(MsgFromIndexing::ActivityChange {
                            running_tasks: actor.running_tasks.len(),
                            queued_tasks: queue.len()
                        });
                        if actor.cancel.is_cancelled() && actor.running_tasks.is_empty() {
                            tracing::debug!("last indexing child task cancelled, shutting down");
                            debug_assert!(!is_running);
                            actor.did_shutdown_send.take().expect("shutdown must only be called once").send(()).expect("receiver must be alive");
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
                tracing::debug!(?msg);
                match msg {
                    MsgToIndexing::Shutdown => {
                        is_running = false;
                        actor.cancel.cancel();
                        if actor.running_tasks.is_empty() {
                            tracing::debug!("no indexing child tasks running, shutting down");
                            actor.did_shutdown_send.take().expect("shutdown must only be called once").send(()).expect("receiver must be alive");
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
                    self.config.bin_paths.clone(),
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
    bin_paths: Option<config::BinPaths>,
    root_dir_id: AssetRootDirId,
    cancel: CancellationToken,
) -> Result<()> {
    let conn = db_pool.get().await?;
    let asset_root = interact!(conn, move |conn| {
        repository::asset_root_dir::get_asset_root(conn, root_dir_id)
    })
    .await?
    .wrap_err("Error getting AssetRootDir from db")?;
    tokio::spawn(async move {
        index_asset_root(db_pool, send_result, bin_paths, asset_root, cancel).await;
    });
    Ok(())
}

#[instrument(skip(pool, send_result, bin_paths))]
async fn index_asset_root(
    pool: DbPool,
    send_result: mpsc::UnboundedSender<(AssetRootDirId, MsgFromIndexing)>,
    bin_paths: Option<config::BinPaths>,
    asset_root: AssetRootDir,
    cancel: CancellationToken,
) {
    tracing::info!(path=%asset_root.path, "Start indexing");
    // TODO WalkDir is synchronous
    // FIXME if a datadir is subdir of assetroot it should obviously not be indexed
    let mut new_asset_count = 0;
    for entry in WalkDir::new(asset_root.path.as_path()).follow_links(true) {
        if cancel.is_cancelled() {
            let _ = send_result.send((
                asset_root.id,
                MsgFromIndexing::IndexingCancelled {
                    root_dir_id: asset_root.id,
                },
            ));
            return;
        }
        match entry {
            Ok(e) => {
                if e.file_type().is_file() {
                    let utf8_path = camino::Utf8Path::from_path(e.path());
                    if let Some(path) = utf8_path {
                        let indexing_res =
                            index_file(path, &asset_root, &pool, bin_paths.as_ref()).await;
                        let msg = match indexing_res {
                            Ok(None) => {
                                continue;
                            }
                            Ok(Some(asset_id)) => {
                                new_asset_count += 1;
                                MsgFromIndexing::NewAsset(asset_id)
                            }
                            Err(report) => MsgFromIndexing::IndexingError {
                                root_dir_id: asset_root.id,
                                path: Some(path.to_owned()),
                                report,
                            },
                        };
                        let _ = send_result.send((asset_root.id, msg));
                    }
                }
            }
            Err(e) => {
                let _ = send_result.send((
                    asset_root.id,
                    MsgFromIndexing::IndexingError {
                        root_dir_id: asset_root.id,
                        path: e.path().map(|p| {
                            p.to_owned()
                                .try_into()
                                .expect("only UTF-8 paths are supported")
                        }),
                        report: eyre!("error while listing directory: {}", e),
                    },
                ));
            }
        }
    }
    let _ = send_result.send((
        asset_root.id,
        MsgFromIndexing::IndexingComplete {
            root_dir_id: asset_root.id,
        },
    ));
    tracing::info!(path=%asset_root.path, new_assets=new_asset_count, "Finished indexing");
}
