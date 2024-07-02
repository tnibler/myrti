use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::{http::Method, Router};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use clap::Parser;
use eyre::{self, Context, Result};
use myrti::{
    app_state::{AppState, SharedState},
    routes,
    spa_serve_dir::SpaServeDirService,
};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::MakeRequestUuid,
    services::ServeDir,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    ServiceBuilderExt,
};
use tracing::info;
use tracing_error::ErrorLayer;
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

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    config: String,
    #[arg(long)]
    skip_startup_check: bool,
    #[cfg(feature = "opentelemetry")]
    #[arg(long)]
    otel_endpoint: Option<String>,
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
    let tracing = tracing_subscriber::registry()
        .with(EnvFilter::from_env("MYRTI_LOG"))
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr));
    #[cfg(feature = "opentelemetry")]
    {
        use opentelemetry_otlp::WithExportConfig;
        let telemetry = args.otel_endpoint.map(|otel_endpoint| {
            let tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .tonic()
                        .with_endpoint(otel_endpoint),
                )
                .with_trace_config(opentelemetry_sdk::trace::config().with_resource(
                    opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                        opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                        "myrti",
                    )]),
                ))
                .install_batch(opentelemetry_sdk::runtime::Tokio)
                .unwrap();
            let _tracer = opentelemetry::global::tracer("myrti");
            tracing_opentelemetry::layer().with_tracer(tracer)
        });
        tracing.with(telemetry).init();
    }
    #[cfg(not(feature = "opentelemetry"))]
    {
        tracing.init();
    }

    core::global_init();
    // TODO make all paths in config absolute relative to config_dir if they're not already
    let config_path = PathBuf::from(args.config);
    let config = core::config::read_config(&config_path).await.unwrap();
    // all paths in config are relative to this
    let config_dir = config_path
        .parent()
        .expect("has read config file, so parent must be a directory");

    if !args.skip_startup_check {
        tracing::info!("Running self check");
        core::startup_self_check::run_self_check(config.bin_paths.as_ref())
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
    let port = config.port.unwrap_or(3000);

    let data_dir_path = if config.data_dir.path.is_absolute() {
        config.data_dir.path.clone()
    } else {
        config_dir.join(&config.data_dir.path)
    };
    let storage_path = data_dir_path.clone();
    info!("Starting up...");
    let pool = db_setup(&data_dir_path).await.unwrap();
    store_asset_roots_from_config(config_dir, &config, &pool).await?;
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
        .nest("/api/timelinegroup", routes::timeline_group::router())
        .nest("/api", routes::api_router())
        .fallback_service(SpaServeDirService::new(ServeDir::new("./static")))
        .layer(
            ServiceBuilder::new()
                .set_x_request_id(MakeRequestUuid)
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
    let listener = tokio::net::TcpListener::bind(SocketAddr::new(addr, port))
        .await
        .wrap_err("Error binding socket")?;
    axum::serve(listener, app)
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
