use crate::{
    core::{monitor::Monitor, scheduler::Scheduler},
    repository::pool::DbPool,
};
use std::sync::{Arc};

pub struct AppState {
    pub pool: DbPool,
    pub scheduler: Scheduler,
    pub monitor: Monitor,
}

pub type SharedState = Arc<AppState>;
