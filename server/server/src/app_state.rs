use std::sync::Arc;

use myrti_core::core::{scheduler::SchedulerHandle, storage::Storage};
use myrti_data::db::DbPool;
use tokio_util::task::TaskTracker;

pub struct AppState {
    pub pool: DbPool,
    pub storage: Storage,
    pub scheduler: SchedulerHandle,
    /// Misc short lived tasks
    pub task_tracker: TaskTracker,
}

pub type SharedState = Arc<AppState>;
