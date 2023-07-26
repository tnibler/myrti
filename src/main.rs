use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use color_eyre::{eyre::Context, owo_colors::colors::xterm::CanCanPink};
use config::Config;
use eyre::{self, bail, Result};
use http_error::HttpError;
use indexing::index_asset_root;
use model::AssetBase;
use repository::pool::DbPool;
use scheduler::SchedulerEvent;
use serde::Deserialize;
use sqlx::{migrate::MigrateDatabase, Executor, Sqlite, SqlitePool};
use std::{path::PathBuf, str::FromStr, sync::Arc};
use tokio::{
    signal,
    sync::{mpsc, Mutex},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
use tracing_error::ErrorLayer;
use tracing_subscriber::{prelude::*, EnvFilter};

use crate::{
    app_state::{AppState, SharedState},
    model::{AssetRootDir, AssetRootDirId},
    scheduler::Scheduler,
};

mod app_state;
mod config;
mod http_error;
mod indexing;
mod indexing_job;
mod model;
mod routes;
mod scheduler;

mod repository {
    pub mod asset;
    pub mod asset_root_dir;
    pub mod pool;
}

async fn db_setup() -> Result<SqlitePool> {
    let db_url = "sqlite://mediathingy.db";
    if !Sqlite::database_exists(db_url).await.unwrap_or(false) {
        println!("creating database");
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
    app_state
        .scheduler
        .send(SchedulerEvent::CancelJob { id: query.0.id })
        .await
        .unwrap();
    Ok(())
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
    let scheduler = Scheduler::start();
    let shared_state: SharedState = Arc::new(AppState {
        pool: pool.clone(),
        scheduler,
    });
    let app = Router::new()
        .nest("/api", routes::api_router())
        .route("/cancel", post(post_cancel))
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
