use crate::{
    core::{monitor::Monitor, scheduler::Scheduler},
    repository::pool::DbPool,
};
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub pool: DbPool,
    pub scheduler: Scheduler,
    pub monitor: Arc<tokio::sync::Mutex<Monitor>>,
}

pub type SharedState = Arc<AppState>;
