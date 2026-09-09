use std::sync::{Arc, Mutex};

use myrti_core::{
    config::Config,
    core::{scheduler::SchedulerHandle, storage::Storage},
};
use myrti_data::db::DbPool;
use tokio_util::task::TaskTracker;

pub struct AppState {
    pub pool: DbPool,
    pub storage: Storage,
    pub scheduler: SchedulerHandle,
    /// Misc short lived tasks
    pub task_tracker: TaskTracker,
    pub config: Arc<Mutex<Config>>,
}

pub type SharedState = Arc<AppState>;
