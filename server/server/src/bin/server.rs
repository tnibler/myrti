use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::{Router, http::Method};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use clap::Parser;
use eyre::{Context, Result, eyre};
use myrti::{
    app_state::{AppState, SharedState},
    routes,
    spa_serve_dir::SpaServeDirService,
};
use tokio::{signal, sync::oneshot};
use tower::ServiceBuilder;
use tower_http::{
    ServiceBuilderExt,
    cors::{Any, CorsLayer},
    request_id::MakeRequestUuid,
    services::{ServeDir, ServeFile},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, info};
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan, prelude::*};

use myrti_core::{
    config::Config,
    core::{
        scheduler::{SchedulerHandle, SchedulerMessage},
        storage::{LocalFileStorage, Storage},
    },
};
use myrti_data::db::DbPool;
use myrti_data::model::{AssetRootDir, AssetRootDirId};
use myrti_data::{db, interact, repository};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    config: String,
    #[arg(long)]
    skip_startup_check: bool,
    /// Serve static web files from this path
    #[arg(long)]
    serve_static: Option<PathBuf>,

    #[arg(short, long)]
    port: Option<u16>,

    /// Pause image/video processing on startup (for development)
    #[arg(long, default_value_t = false)]
    pause_processing: bool,
    /// Pause video processing on startup (for development)
    #[arg(long, default_value_t = false)]
    pause_video_processing: bool,
}

async fn db_setup(dir: &Path) -> Result<DbPool> {
    let db_url = dir.join("myrti_media.db").to_string();
    let pool = db::open_db_pool(&db_url)?;
    let conn = pool.get().await?;
    interact!(conn, db::migrate).await??;
    Ok(pool)
}

async fn store_asset_roots_from_config(
    config_dir: &Path,
    config: &Config,
    pool: &DbPool,
) -> Result<()> {
    let conn = pool.get().await?;
    for asset_dir in config.asset_dirs.iter() {
        let asset_dir_path = if asset_dir.path.is_absolute() {
            asset_dir.path.to_owned()
        } else {
            config_dir.join(&asset_dir.path)
        };
        // FIXME: this does not handle paths that differ in characters but point to the same
        // location correctly. The path-clean crate or this function from cargo would do the
        // job: https://github.com/rust-lang/cargo/blob/fede83ccf973457de319ba6fa0e36ead454d2e20/src/cargo/util/paths.rs#L61
        let existing = interact!(conn, move |conn| {
            repository::asset_root_dir::get_asset_root_with_path(conn, &asset_dir_path)
        })
        .await?
        .wrap_err("Error checkng if AssetRootDir already exists")?;
        if existing.is_none() {
            let asset_dir_path = asset_dir.path.to_owned();
            interact!(conn, move |conn| {
                repository::asset_root_dir::insert_asset_root(
                    conn,
                    &AssetRootDir {
                        id: AssetRootDirId(0),
                        path: asset_dir_path,
                    },
                )
            })
            .await??;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    if std::env::var("RUST_SPANTRACE").is_err() {
        unsafe {
            std::env::set_var("RUST_SPANTRACE", "1");
        }
    }
    color_eyre::install()?;
    if std::env::var("MYRTI_LOG").is_err() {
        unsafe { std::env::set_var("MYRTI_LOG", "info") }
    }
    let file_appender = tracing_appender::rolling::never("/tmp/", "myrti.log");
    let (non_blocking_appender, _guard) = tracing_appender::non_blocking(file_appender);
    let tracing = tracing_subscriber::registry()
        .with(EnvFilter::from_env("MYRTI_LOG"))
        .with(ErrorLayer::default())
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(false)
                .with_file(false)
                .with_line_number(false)
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(std::io::stderr),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_file(false)
                .with_line_number(false)
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(non_blocking_appender),
        );
    tracing.init();

    myrti_core::global_init();
    // TODO make all paths in config absolute relative to config_dir if they're not already
    let config_path = PathBuf::from(args.config);
    let config = myrti_core::config::read_config(&config_path)
        .await
        .wrap_err_with(|| format!("error parsing config file {config_path}"))?;
    // all paths in config are relative to this
    let config_dir = config_path
        .parent()
        .expect("has read config file, so parent must be a directory");

    let data_dir_path = if config.data_dir.path.is_absolute() {
        config.data_dir.path.clone()
    } else {
        config_dir.join(&config.data_dir.path)
    };
    if !std::fs::exists(&data_dir_path)? {
        std::fs::create_dir(&data_dir_path)
            .with_context(|| format!("error creating data directory at {}", data_dir_path))?;
    }
    let db_path = config.data_dir.db_path.as_deref().unwrap_or(&data_dir_path);
    if !std::fs::exists(db_path)? {
        std::fs::create_dir(db_path)
            .with_context(|| format!("error creating database directory at {}", db_path))?;
    }

    let data_dir_canon = match config.data_dir.path.canonicalize_utf8() {
        Ok(p) => p,
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                match config.data_dir.path.parent().map(|p| p.canonicalize_utf8()) {
                    Some(Ok(p)) => p,
                    Some(Err(e)) => return Err(eyre!("error reading data directory path: {}", e)),
                    None => return Err(eyre!("data directory path does not exist: {}", e)),
                }
            }
            _ => return Err(eyre!("error reading data directory path: {}", e)),
        },
    };
    if let Some(bad_dir) = config.asset_dirs.iter().find(|dir| {
        dir.path
            .canonicalize_utf8()
            .is_ok_and(|dir| dir.starts_with(&data_dir_canon))
    }) {
        return Err(eyre!(
            "data directory {data_dir_canon} can not be subdirectory of asset directory {}",
            bad_dir.path
        ));
    }

    info!("Starting up...");

    if !args.skip_startup_check {
        tracing::info!("Running self check");
        myrti_core::startup_self_check::run_self_check(config.bin_paths.as_ref())
            .await
            .expect("Self check failed");
        tracing::info!("Self check successful");
    } else {
        tracing::info!("Skipping self check");
    }

    let addr: IpAddr = config
        .address
        .as_ref()
        .map(|a| a.parse().wrap_err("error parsing listening address"))
        .transpose()?
        .unwrap_or("127.0.0.1".parse().expect("is a valid address"));
    let port = args.port.unwrap_or(3000);

    let pmtiles_path = data_dir_path.join("map.pmtiles");

    let pool = db_setup(db_path).await.unwrap();
    store_asset_roots_from_config(config_dir, &config, &pool).await?;
    let storage: Storage = LocalFileStorage::new(data_dir_path).into();
    let (scheduler_did_shutdown_send, scheduler_did_shutdown_recv) = oneshot::channel();
    let scheduler = SchedulerHandle::new(
        pool.clone(),
        storage.clone(),
        config,
        scheduler_did_shutdown_send,
    );

    if args.pause_processing {
        scheduler
            .send
            .send(SchedulerMessage::PauseAllProcessing)
            .await
            .expect("scheduler must be alive")
    } else if args.pause_video_processing {
        scheduler
            .send
            .send(SchedulerMessage::PauseVideoPackaging)
            .await
            .expect("scheduler must be alive")
    }
    scheduler
        .send
        .send(SchedulerMessage::Startup)
        .await
        .expect("scheduler must be alive");

    let shared_state: SharedState = Arc::new(AppState {
        pool: pool.clone(),
        storage,
        scheduler: scheduler.clone(),
    });
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any);
    let app = Router::new()
        .nest("/api/timeline", routes::timeline::router())
        .nest("/api/albums", routes::album::router())
        .nest("/api/files", routes::file::router())
        .nest("/api/assets", routes::asset::router())
        .nest("/api/photoSeries", routes::photo_series::router())
        .nest("/api/assetRoots", routes::asset_roots::router())
        .nest("/api/dash", routes::dash::router())
        .nest("/api/timelinegroups", routes::timeline_group::router())
        .nest("/api/jobs", routes::jobs::router())
        .nest("/api/map", routes::map::router())
        .nest("/api", routes::api_router())
        .nest_service("/static/map.pmtiles", ServeFile::new(&pmtiles_path));

    let app = match args.serve_static.as_deref() {
        Some(static_path) => {
            tracing::debug!(?static_path, "serving static files");
            app.fallback_service(SpaServeDirService::new(
                ServeDir::new(static_path).fallback(ServeFile::new(static_path.join("index.html"))),
            ))
        }
        None => app,
    }
    .layer(
        ServiceBuilder::new()
            .set_x_request_id(MakeRequestUuid)
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(
                        DefaultMakeSpan::new()
                            .level(Level::TRACE)
                            .include_headers(false),
                    )
                    .on_request(())
                    .on_response(
                        DefaultOnResponse::new()
                            .level(Level::TRACE)
                            .include_headers(false),
                    ),
            ),
    )
    .layer(cors)
    .with_state(shared_state);
    // .route("/api/assets", get(get_assets))
    // .route("/api/assetRoots", get(get_asset_roots))
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(addr, port))
        .await
        .wrap_err("Error binding socket")?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    info!("Shutting down");
    scheduler
        .send
        .send(SchedulerMessage::Shutdown)
        .await
        .expect("scheduler must be alive");
    info!("Waiting for shutdown...");
    scheduler_did_shutdown_recv
        .await
        .expect("scheduler must be alive");
    myrti_core::processing::image::vips_teardown();
    Ok(())
}

async fn shutdown_signal() {
    match signal::ctrl_c().await {
        Ok(()) => {
            tracing::debug!("Received ctrl-c shutdown signal");
        }
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
            std::process::exit(1);
            // we also shut down in case of error
        }
    }
}
