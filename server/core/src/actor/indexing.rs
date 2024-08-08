use std::collections::VecDeque;

use camino::Utf8PathBuf as PathBuf;
use eyre::{eyre, Context, Result};
use tokio::sync::mpsc;
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
    FailedToStartIndexing {
        root_dir_id: AssetRootDirId,
        report: eyre::Report,
    },
}

#[derive(Debug, Clone)]
enum MsgToIndexing {
    Pause,
    Resume,
    DoTask(DoTaskMsg),
    Shutdown,
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
        let actor = IndexingActor {
            db_pool,
            config,
            send_from_us,
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
}

struct IndexingActor {
    pub db_pool: DbPool,
    pub config: config::Config,
    pub send_from_us: mpsc::UnboundedSender<MsgFromIndexing>,
}

const MAX_TASKS: usize = 4;
const MAX_QUEUE_SIZE: usize = 10;

async fn run_indexing_actor(
    mut recv: mpsc::UnboundedReceiver<MsgToIndexing>,
    actor: IndexingActor,
) {
    let mut is_running = true;
    let mut running_tasks: usize = 0;
    let mut queue: VecDeque<DoTaskMsg> = Default::default();
    loop {
        tokio::select! {
            Some(msg) = recv.recv() => {
                match msg {
                    MsgToIndexing::Pause => {
                        is_running = false;
                        // TODO: pause currently running indexing jobs
                    }
                    MsgToIndexing::Resume => {
                        is_running = true;
                        // TODO: unpause currently running indexing jobs
                    }
                    MsgToIndexing::DoTask(task) => {
                        if is_running && running_tasks < MAX_TASKS {
                            running_tasks += 1;
                            let _ = actor.send_from_us.send(MsgFromIndexing::ActivityChange {
                                running_tasks,
                                queued_tasks: queue.len()
                            });
                            actor.process_message(task).await;
                        } else if queue.len() < MAX_QUEUE_SIZE {
                            queue.push_back(task);
                            let _ = actor.send_from_us.send(MsgFromIndexing::ActivityChange {
                                running_tasks,
                                queued_tasks: queue.len()
                            });
                        } else {
                            let _ = actor.send_from_us.send(MsgFromIndexing::DroppedMessage);
                        }
                    }
                    MsgToIndexing::Shutdown => {
                    }
                }
            }
        }
    }
}

impl IndexingActor {
    async fn process_message(&self, msg: DoTaskMsg) {
        match msg {
            DoTaskMsg::IndexAssetRootDir { root_dir_id } => {
                let send_copy = self.send_from_us.clone();

                let start_result = handle_indexing_message(
                    self.db_pool.clone(),
                    send_copy,
                    self.config.bin_paths.clone(),
                    root_dir_id,
                )
                .await;

                if let Err(report) = start_result {
                    let _ = self
                        .send_from_us
                        .send(MsgFromIndexing::FailedToStartIndexing {
                            root_dir_id,
                            report: report.wrap_err("Error starting indexing job"),
                        });
                }
            }
        }
    }
}

async fn handle_indexing_message(
    db_pool: DbPool,
    send_result: mpsc::UnboundedSender<MsgFromIndexing>,
    bin_paths: Option<config::BinPaths>,
    root_dir_id: AssetRootDirId,
) -> Result<()> {
    let conn = db_pool.get().await?;
    let asset_root = interact!(conn, move |conn| {
        repository::asset_root_dir::get_asset_root(conn, root_dir_id)
    })
    .await?
    .wrap_err("Error getting AssetRootDir from db")?;
    tokio::spawn(async move {
        index_asset_root(db_pool, send_result, bin_paths, asset_root).await;
    });
    Ok(())
}

#[instrument(skip(pool, send_result, bin_paths))]
async fn index_asset_root(
    pool: DbPool,
    send_result: mpsc::UnboundedSender<MsgFromIndexing>,
    bin_paths: Option<config::BinPaths>,
    asset_root: AssetRootDir,
) {
    tracing::info!(path=%asset_root.path, "Start indexing");
    // TODO WalkDir is synchronous
    // FIXME if a datadir is subdir of assetroot it should obviously not be indexed
    let mut new_asset_count = 0;
    for entry in WalkDir::new(asset_root.path.as_path()).follow_links(true) {
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
                        let _ = send_result.send(msg);
                    }
                }
            }
            Err(e) => {
                let _ = send_result.send(MsgFromIndexing::IndexingError {
                    root_dir_id: asset_root.id,
                    path: e.path().map(|p| {
                        p.to_owned()
                            .try_into()
                            .expect("only UTF-8 paths are supported")
                    }),
                    report: eyre!("error while listing directory: {}", e),
                });
            }
        }
    }
    tracing::info!(path=%asset_root.path, new_assets=new_asset_count, "Finished indexing");
}
