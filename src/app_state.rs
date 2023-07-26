use crate::{repository::pool::DbPool, scheduler::Scheduler};
use std::sync::Arc;

pub struct AppState {
    pub pool: DbPool,
    pub scheduler: Scheduler,
}

pub type SharedState = Arc<AppState>;
