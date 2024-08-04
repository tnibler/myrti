use core::core::scheduler::SchedulerMessage;

use axum::{extract::State, routing::post, Router};
use eyre::Context;

use crate::{app_state::SharedState, http_error::ApiResult};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/pauseAll", post(pause_all_jobs))
        .route("/resumeAll", post(resume_all_jobs))
}

async fn pause_all_jobs(State(app_state): State<SharedState>) -> ApiResult<()> {
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::PauseAllJobs)
        .await
        .wrap_err("error sending message to scheduler")?;
    Ok(())
}

async fn resume_all_jobs(State(app_state): State<SharedState>) -> ApiResult<()> {
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::ResumeAllJobs)
        .await
        .wrap_err("error sending message to scheduler")?;
    Ok(())
}
