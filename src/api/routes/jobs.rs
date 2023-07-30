use axum::{extract::State, routing::get, Json, Router};

use crate::{
    api::{
        schema::{api_job_status, api_job_type, JobId, JobInfo},
        ApiResult,
    },
    app_state::{self, SharedState},
};

pub fn router() -> Router<SharedState> {
    Router::new().route("/", get(get_all_jobs))
}

pub async fn get_all_jobs(app_state: State<SharedState>) -> ApiResult<Json<Vec<JobInfo>>> {
    let jobs = app_state.monitor.get_all_jobs().await?;
    let dtos: Vec<JobInfo> = jobs
        .into_iter()
        .map(|job_info| JobInfo {
            id: job_info.id.into(),
            ty: api_job_type(&job_info.ty),
            status: job_info.status.as_ref().map(|s| api_job_status(s)),
        })
        .collect();
    Ok(Json(dtos))
}
