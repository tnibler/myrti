use crate::{
    core::{monitor::Monitor, scheduler::Scheduler, storage::Storage},
    repository::pool::DbPool,
};
use std::sync::Arc;

pub struct AppState {
    pub pool: DbPool,
    pub storage: Storage,
    pub scheduler: Scheduler,
    pub monitor: Monitor,
}

pub type SharedState = Arc<AppState>;
