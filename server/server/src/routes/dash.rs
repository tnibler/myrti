use axum::{
    body::Body,
    extract::{Path, State},
    http::Request,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use eyre::Context;
use serde::Deserialize;
use tower::ServiceExt;
use tracing::Instrument;

use core::{catalog::storage_key, core::storage::StorageProvider, model};

use crate::{app_state::SharedState, http_error::ApiResult, schema::AssetId};

pub fn router() -> Router<SharedState> {
    Router::new().route("/:id/*path", get(get_dash_file).options(get_dash_file))
}

#[derive(Debug, Clone, Deserialize)]
struct DashFilePath {
    pub id: String,
    pub path: String,
}

#[tracing::instrument(fields(request = true), skip(app_state))]
async fn get_dash_file(
    Path(path): Path<DashFilePath>,
    State(app_state): State<SharedState>,
    request: Request<Body>,
) -> ApiResult<Response> {
    let asset_id: model::AssetId = AssetId(path.id).try_into()?;

    let storage_key = storage_key::dash_file(asset_id, format_args!("{}", &path.path));
    // TODO (#8)
    // TODO handle non-local StorageProvider
    // TODO return correct error code for not found
    // let content_type = match &path.path {
    //     path if path.ends_with("mp4") => "video/mp4",
    //     path if path.ends_with("stream.mpd") => "application/octet-stream",
    //     _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    // };
    // let read = app_state.storage.open_read_stream(&storage_key).await?;
    // let headers = [(CONTENT_TYPE, content_type)];
    let path = app_state
        .storage
        .local_path(&storage_key)
        .await?
        .expect("not implemented for non-local StorageProvider");
    let serve_dir = tower_http::services::ServeFile::new(&path)
        .oneshot(request)
        .await
        .wrap_err("error serving file")?;
    Ok(serve_dir.into_response())
}
