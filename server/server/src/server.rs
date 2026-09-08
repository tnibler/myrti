use std::sync::{Arc, Mutex};

use axum::{Router, http::Method};
use camino::Utf8Path as Path;
use eyre::{Context, Result};
use myrti_core::{
    config::Config,
    core::{
        scheduler::{MessageFromScheduler, SchedulerHandle},
        storage::Storage,
    },
};
use myrti_data::{
    db::{self, DbPool},
    interact,
    model::{self, AssetRootDir},
    repository,
};
use tokio::sync::broadcast;
use tokio_util::task::TaskTracker;
use tower_http::cors::CorsLayer;

use crate::{
    app_state::{AppState, SharedState},
    routes,
};

pub struct Server {
    pub app: Router<()>,
    pub scheduler: SchedulerHandle,
    pub scheduler_recv: broadcast::Receiver<MessageFromScheduler>,
    pub db_pool: DbPool,
    pub task_tracker: TaskTracker,
}

pub struct SetupConfig<'a> {
    pub config: Arc<Mutex<Config>>,
    pub config_dir: &'a Path,
    pub db_url: &'a str,
    pub storage: Storage,
}

async fn db_setup(db_url: &str) -> Result<DbPool> {
    let pool = db::open_db_pool(db_url)?;
    let conn = pool.get().await?;
    interact!(conn, db::migrate).await??;
    Ok(pool)
}

pub async fn make_app(
    SetupConfig {
        config,
        config_dir,
        db_url,
        storage,
    }: SetupConfig<'_>,
) -> Result<Server> {
    myrti_core::global_init();

    let pool = db_setup(db_url)
        .await
        .wrap_err("error setting up database")?;
    store_asset_roots_from_config(config_dir, &config.lock().unwrap(), &pool).await?;

    let (scheduler, scheduler_recv) = SchedulerHandle::new(pool.clone(), storage.clone(), config);
    let task_tracker = TaskTracker::new();
    let shared_state: SharedState = Arc::new(AppState {
        pool: pool.clone(),
        storage,
        scheduler: scheduler.clone(),
        task_tracker: task_tracker.clone(),
    });
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(tower_http::cors::Any);
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
        .layer(cors)
        .with_state(shared_state);

    Ok(Server {
        app,
        scheduler,
        scheduler_recv,
        db_pool: pool,
        task_tracker,
    })
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
        .wrap_err("Error checking if AssetRootDir already exists")?;
        if existing.is_none() {
            let asset_dir_path = asset_dir.path.to_owned();
            interact!(conn, move |conn| {
                repository::asset_root_dir::insert_asset_root(
                    conn,
                    &AssetRootDir {
                        id: model::AssetRootDirId(0),
                        path: asset_dir_path,
                    },
                )
            })
            .await??;
        }
    }
    Ok(())
}
