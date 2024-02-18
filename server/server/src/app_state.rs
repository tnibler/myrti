use std::sync::Arc;

use core::{
    core::{scheduler::SchedulerHandle, storage::Storage},
    model::repository::db::DbPool,
};

pub struct AppState {
    pub pool: DbPool,
    pub storage: Storage,
    pub scheduler: SchedulerHandle,
}

pub type SharedState = Arc<AppState>;
