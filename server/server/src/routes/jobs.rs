use core::core::scheduler::SchedulerMessage;

use axum::{extract::State, routing::post, Router};
use eyre::Context;

use crate::{app_state::SharedState, http_error::ApiResult};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/pauseAllProcessing", post(pause_all_processing))
        .route("/resumeAllProcessing", post(resume_all_processing))
        .route("/pauseVideoProcessing", post(pause_video_processing))
        .route("/resumeVideoProcessing", post(resume_video_processing))
}

async fn pause_all_processing(State(app_state): State<SharedState>) -> ApiResult<()> {
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::PauseAllProcessing)
        .await
        .wrap_err("error sending message to scheduler")?;
    Ok(())
}

async fn resume_all_processing(State(app_state): State<SharedState>) -> ApiResult<()> {
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::ResumeAllProcessing)
        .await
        .wrap_err("error sending message to scheduler")?;
    Ok(())
}

async fn pause_video_processing(State(app_state): State<SharedState>) -> ApiResult<()> {
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::PauseVideoPackaging)
        .await
        .wrap_err("error sending message to scheduler")?;
    Ok(())
}

async fn resume_video_processing(State(app_state): State<SharedState>) -> ApiResult<()> {
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::ResumeVideoPackaging)
        .await
        .wrap_err("error sending message to scheduler")?;
    Ok(())
}
