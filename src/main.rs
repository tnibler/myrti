use crate::core::{
    job::JobId,
    monitor::{Monitor, MonitorMessage},
};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use color_eyre::eyre::Context;
use config::Config;
use eyre::{self, Result};
use http_error::HttpError;
use repository::pool::DbPool;
use serde::Deserialize;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use std::{path::PathBuf, str::FromStr, sync::Arc};
use tokio::{signal, sync::mpsc};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter};

use crate::{
    app_state::{AppState, SharedState},
    core::scheduler::Scheduler,
    model::{AssetRootDir, AssetRootDirId},
};

mod app_state;
mod config;
mod core;
mod http_error;
mod indexing;
mod job;
mod model;
mod routes;

mod repository {
    pub mod asset;
    pub mod asset_root_dir;
    pub mod pool;
}

async fn db_setup() -> Result<SqlitePool> {
    let db_url = "sqlite://mediathingy.db";
    if !Sqlite::database_exists(db_url).await.unwrap_or(false) {
        info!("Creating database {}", db_url);
        Sqlite::create_database(db_url).await?;
    }
    // } else {
    //     println!("dropping and recreating database");
    //     Sqlite::drop_database(db_url).await?;
    //     Sqlite::create_database(db_url).await?;
    // }

    let pool = SqlitePool::connect(db_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

async fn store_asset_roots_from_config(config: &Config, pool: &DbPool) -> Result<()> {
    for asset_dir in config.asset_dirs.iter() {
        let existing = repository::asset_root_dir::get_asset_root_with_path(pool, &asset_dir.path)
            .await
            .wrap_err("Error checkng if AssetRootDir already exists")?;
        if existing.is_none() {
            repository::asset_root_dir::insert_asset_root(
                &pool,
                AssetRootDir {
                    id: AssetRootDirId(0),
                    path: asset_dir.path.clone(),
                },
            )
            .await?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct QueryCancel {
    id: u64,
}

async fn post_cancel(
    query: Query<QueryCancel>,
    app_state: State<SharedState>,
) -> Result<impl IntoResponse, HttpError> {
    app_state.monitor.cancel_job(JobId(query.0.id)).await?;
    Ok(())
}

async fn get_status(
    query: Query<QueryCancel>,
    app_state: State<SharedState>,
) -> Result<impl IntoResponse, HttpError> {
    let status = app_state.monitor.get_status(JobId(query.id)).await?;
    Ok(format!("{:#?}", status))
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug,hyper=info")
    }
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    info!("Starting up...");
    let config = config::read_config(PathBuf::from_str("config.toml").unwrap().as_path())
        .await
        .unwrap();
    let pool = db_setup().await.unwrap();
    store_asset_roots_from_config(&config, &pool).await?;
    // run it with hyper on localhost:3000
    let (monitor_tx, monitor_rx) = mpsc::channel::<MonitorMessage>(1000);
    let scheduler = Scheduler::start(monitor_tx, pool.clone());
    let monitor_cancel = CancellationToken::new();
    let monitor = Monitor::new(monitor_rx, scheduler.tx.clone(), monitor_cancel.clone());
    let shared_state: SharedState = Arc::new(AppState {
        pool: pool.clone(),
        scheduler,
        monitor,
    });
    let app = Router::new()
        .nest("/api", routes::api_router())
        .route("/cancel", post(post_cancel))
        .route("/status", get(get_status))
        .with_state(shared_state);
    // .route("/api/assets", get(get_assets))
    // .route("/api/assetRoots", get(get_asset_roots))
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    info!("Shutting down...");

    pool.close().await;
    Ok(())
}

async fn shutdown_signal() {
    match signal::ctrl_c().await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            std::process::exit(1);
            // we also shut down in case of error
        }
    }
}
