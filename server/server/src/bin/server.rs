use std::{
    net::{IpAddr, SocketAddr},
    sync::Mutex,
};

use camino::Utf8PathBuf as PathBuf;
use clap::Parser;
use eyre::{Context, Result, eyre};
use myrti::{
    server::{Server, SetupConfig, make_app},
    spa_serve_dir::SpaServeDirService,
};
use tokio::{signal, sync::broadcast::error::RecvError};
use tower::ServiceBuilder;
use tower_http::{
    ServiceBuilderExt,
    request_id::MakeRequestUuid,
    services::{ServeDir, ServeFile},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, info};
use tracing_error::ErrorLayer;
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan, prelude::*};

use myrti_core::core::{
    scheduler::{MessageFromScheduler, SchedulerMessage},
    storage::Storage,
};

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

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

    // TODO make all paths in config absolute relative to config_dir if they're not already
    let config_path = PathBuf::from(args.config);
    let config = myrti_core::config::read_config(&config_path)
        .await
        .wrap_err_with(|| format!("error parsing config file {config_path}"))?;
    // all paths in config are relative to this
    let config_dir = config_path
        .parent()
        .expect("has read config file, so parent must be a directory");

    let data_dir = if config.data_dir.path.is_absolute() {
        config.data_dir.path.clone()
    } else {
        config_dir.join(&config.data_dir.path)
    };
    if !std::fs::exists(&data_dir)? {
        std::fs::create_dir(&data_dir)
            .with_context(|| format!("error creating data directory at {}", data_dir))?;
    }
    let db_path = config.data_dir.db_path.as_deref().unwrap_or(&data_dir);
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
    let pmtiles_path = data_dir.join("map.pmtiles");
    let db_url = db_path.join("myrti_media.db");
    let storage = Storage::new(data_dir);
    let Server {
        app,
        scheduler,
        mut scheduler_recv,
        ..
    } = make_app(SetupConfig {
        config: Mutex::new(config).into(),
        storage,
        config_dir,
        db_url: db_url.as_str(),
    })
    .await?;

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

    let app = app
        .nest_service("/static/map.pmtiles", ServeFile::new(&pmtiles_path))
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
        );
    let app = match args.serve_static.as_deref() {
        Some(static_path) => {
            tracing::debug!(?static_path, "serving static files");
            app.fallback_service(SpaServeDirService::new(
                ServeDir::new(static_path).fallback(ServeFile::new(static_path.join("index.html"))),
            ))
        }
        None => app,
    };

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
    loop {
        let received = scheduler_recv.recv().await;
        match received {
            Err(RecvError::Closed) => break,
            Err(err) => {
                tracing::error!(?err);
            }
            Ok(_msg) => {}
        }
    }
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
