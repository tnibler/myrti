use std::sync::Arc;

use core::{
    core::{monitor::Monitor, scheduler::Scheduler, storage::Storage},
    model::repository::pool::DbPool,
};

pub struct AppState {
    pub pool: DbPool,
    pub storage: Storage,
    pub scheduler: Scheduler,
    pub monitor: Monitor,
}

pub type SharedState = Arc<AppState>;
