use std::{str::FromStr, sync::Arc};

use axum::{
    extract::{Query, State},
    http::Method,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use camino::Utf8PathBuf as PathBuf;
use eyre::{self, Context, Result};
use mediathingyrust::{
    app_state::{AppState, SharedState},
    http_error::HttpError,
    routes,
    spa_serve_dir::SpaServeDirService,
};
use serde::Deserialize;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::MakeRequestUuid,
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    ServiceBuilderExt,
};
use tracing::{info, instrument, Instrument};
use tracing_error::ErrorLayer;
use tracing_forest::ForestLayer;
use tracing_subscriber::{prelude::*, EnvFilter};

use core::{
    config::Config,
    core::{
        scheduler::SchedulerHandle,
        storage::{LocalFileStorage, Storage},
    },
    deadpool_diesel, interact,
    model::{
        repository::{
            self,
            db::{self, DbPool},
        },
        AssetRootDir, AssetRootDirId,
    },
};

async fn db_setup() -> Result<DbPool> {
    let db_url = "mediathingy.db";
    let pool = db::open_db_pool(db_url)?;
    let conn = pool.get().await?;
    interact!(conn, move |mut conn| db::migrate(&mut conn))
        .in_current_span()
        .await??;
    Ok(pool)
}

async fn store_asset_roots_from_config(config: &Config, pool: &DbPool) -> Result<()> {
    let conn = pool.get().in_current_span().await?;
    for asset_dir in config.asset_dirs.iter() {
        let asset_dir_path = asset_dir.path.to_owned();
        let existing = interact!(conn, move |mut conn| {
            repository::asset_root_dir::get_asset_root_with_path(&mut conn, &asset_dir_path)
        })
        .in_current_span()
        .await?
        .wrap_err("Error checkng if AssetRootDir already exists")?;
        if existing.is_none() {
            let asset_dir_path = asset_dir.path.to_owned();
            interact!(conn, move |mut conn| {
                repository::asset_root_dir::insert_asset_root(
                    &mut conn,
                    &AssetRootDir {
                        id: AssetRootDirId(0),
                        path: asset_dir_path,
                    },
                )
            })
            .in_current_span()
            .await??;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    if std::env::var("RUST_SPANTRACE").is_err() {
        std::env::set_var("RUST_SPANTRACE", "1");
    }
    color_eyre::install()?;
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug,hyper=info")
    }
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(ErrorLayer::default())
        .with(ForestLayer::default())
        // .with(
        //     tracing_subscriber::fmt::layer()
        //         // .with_span_events(FmtSpan::NEW)
        //         .with_writer(std::io::stderr),
        // )
        .init();

    info!("Starting up...");
    core::global_init();
    let config =
        core::config::read_config(PathBuf::from_str("server/config.toml").unwrap().as_path())
            .await
            .unwrap();
    let pool = db_setup().await.unwrap();
    store_asset_roots_from_config(&config, &pool).await?;
    let storage_path = config.data_dir.path.clone();
    std::fs::create_dir_all(&storage_path).unwrap();
    let storage: Storage = LocalFileStorage::new(storage_path).into();
    let scheduler = SchedulerHandle::new(pool.clone(), storage.clone(), config);
    let shared_state: SharedState = Arc::new(AppState {
        pool: pool.clone(),
        storage,
        scheduler,
    });
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any);
    let app = Router::new()
        .nest("/api/timeline", routes::timeline::router())
        .nest("/api/albums", routes::album::router())
        .nest("/api/asset", routes::asset::router())
        .nest("/api/assetRoots", routes::asset_roots::router())
        .nest("/api/dash", routes::dash::router())
        .nest("/api", routes::api_router())
        .fallback_service(SpaServeDirService::new(ServeDir::new("../web/build")))
        .layer(
            ServiceBuilder::new()
                .set_x_request_id(MakeRequestUuid::default())
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().include_headers(true))
                        .on_response(DefaultOnResponse::new().include_headers(true)),
                ),
        )
        .layer(cors)
        .with_state(shared_state);
    // .route("/api/assets", get(get_assets))
    // .route("/api/assetRoots", get(get_asset_roots))
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    info!("Shutting down...");

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
