use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use eyre::{eyre, Context, Result};
use tokio::sync::{mpsc, watch};
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

#[derive(Debug, Clone)]
pub enum IndexingMessage {
    IndexAssetRootDir { root_dir_id: AssetRootDirId },
}

#[derive(Debug)]
pub enum IndexingResult {
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

#[derive(Debug)]
pub struct IndexingActorHandle {
    pub send: mpsc::Sender<IndexingMessage>,
    pub recv_result: mpsc::Receiver<IndexingResult>,
    /// Set when the channel for IndexingResult messages is full and result messages
    /// are dropped. Signals to the result consumer that it should poll the db for any
    /// missed new assets.
    pub recv_has_dropped_results: watch::Receiver<Option<usize>>,
}

impl IndexingActorHandle {
    pub fn new(db_pool: DbPool, config: config::Config) -> Self {
        let (send, recv) = mpsc::channel(1000);
        let (send_result, recv_result) = mpsc::channel(10000);
        let (send_has_dropped_results, recv_has_dropped_results) = watch::channel(None);
        let actor = IndexingActor {
            db_pool,
            config,
            recv,
            send: send_result,
            send_has_dropped_results,
        };
        tokio::spawn(run_indexing_actor(actor));
        Self {
            send,
            recv_result,
            recv_has_dropped_results,
        }
    }
}

struct IndexingActor {
    pub db_pool: DbPool,
    pub config: config::Config,
    pub recv: mpsc::Receiver<IndexingMessage>,
    pub send: mpsc::Sender<IndexingResult>,
    pub send_has_dropped_results: watch::Sender<Option<usize>>,
}

#[instrument(skip(actor), level = "debug")]
async fn run_indexing_actor(mut actor: IndexingActor) {
    // TODO deduplicate job requests in actors where necessary
    let mut running_jobs: HashSet<AssetRootDirId> = HashSet::default();
    // Passed to the tasks actually doing the indexing.
    // Bounded and without the logic to signal dropped result messages because
    // this actor loop here doesn't do much and there's really no reason for the channel
    // to fill up.
    let (send_result, mut recv_result) = mpsc::channel(10000);
    let mut dropped_results = 0;
    loop {
        tokio::select! {
            Some(msg) = actor.recv.recv() => {
                match msg {
                    IndexingMessage::IndexAssetRootDir { root_dir_id } => {
                        let send_result_copy = send_result.clone();
                        let bin_paths = actor.config.bin_paths.clone();
                        let start_result = handle_indexing_message(actor.db_pool.clone(), send_result_copy, bin_paths, root_dir_id).await;
                        if let Err(report) = start_result {
                            let _ = actor.send.try_send(IndexingResult::FailedToStartIndexing {
                                root_dir_id,
                                report: report.wrap_err("Error starting indexing job")
                            });
                        }
                    }
                }
            }
            Some(result) = recv_result.recv() => {
                let is_success = matches!(result, IndexingResult::NewAsset(_));
                match actor.send.try_send(result) {
                    Err(_) if is_success => {
                        dropped_results += 1;
                        let _ = actor.send_has_dropped_results.send(Some(dropped_results));
                    },
                    Err(_)| Ok(_) => { /* do nothing */ }
                }
            }
        }
    }
}

async fn handle_indexing_message(
    db_pool: DbPool,
    send_result: mpsc::Sender<IndexingResult>,
    bin_paths: Option<config::BinPaths>,
    root_dir_id: AssetRootDirId,
) -> Result<()> {
    let conn = db_pool.get().await?;
    let asset_root = interact!(conn, move |mut conn| {
        repository::asset_root_dir::get_asset_root(&mut conn, root_dir_id)
    })
    .await?
    .wrap_err("Error getting AssetRootDir from db")?;
    tokio::spawn(async move {
        index_asset_root(db_pool, send_result, bin_paths.as_ref(), asset_root).await;
    });
    Ok(())
}

#[instrument(skip(pool, send_result, bin_paths), level = "debug")]
async fn index_asset_root(
    pool: DbPool,
    send_result: mpsc::Sender<IndexingResult>,
    bin_paths: Option<&config::BinPaths>,
    asset_root: AssetRootDir,
) {
    // TODO WalkDir is synchronous
    // FIXME if a datadir is subdir of assetroot it should obviously not be indexed
    for entry in WalkDir::new(asset_root.path.as_path()).follow_links(true) {
        match entry {
            Ok(e) => {
                if e.file_type().is_file() {
                    let utf8_path = camino::Utf8Path::from_path(e.path());
                    if let Some(path) = utf8_path {
                        let indexing_res = index_file(path, &asset_root, &pool, bin_paths).await;
                        let msg = match indexing_res {
                            Ok(None) => {
                                continue;
                            }
                            Ok(Some(asset_id)) => IndexingResult::NewAsset(asset_id),
                            Err(report) => IndexingResult::IndexingError {
                                root_dir_id: asset_root.id,
                                path: Some(path.to_owned()),
                                report,
                            },
                        };
                        let _ = send_result.send(msg).await.unwrap();
                    }
                }
            }
            Err(e) => {
                let _ = send_result
                    .send(IndexingResult::IndexingError {
                        root_dir_id: asset_root.id,
                        path: e.path().map(|p| {
                            p.to_owned()
                                .try_into()
                                .expect("only UTF-8 paths are supported")
                        }),
                        report: eyre!("error while listing directory: {}", e),
                    })
                    .await;
            }
        }
    }
}
