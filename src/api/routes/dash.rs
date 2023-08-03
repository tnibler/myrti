use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, options},
    Router,
};
use serde::Deserialize;
use tower::ServiceExt;
use tracing::{debug, instrument, Instrument};

use crate::{
    api::{schema::AssetId, ApiResult},
    app_state::{self, SharedState},
    model::{self, repository, AssetType},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/:id/:file_name", get(get_dash_file))
        .route("/:id/:file_name", options(get_dash_file))
}

#[derive(Debug, Clone, Deserialize)]
struct DashFilePath {
    pub id: String,
    pub file_name: String,
}

#[instrument(name = "Get Dash file", skip(app_state))]
async fn get_dash_file(
    Path(path): Path<DashFilePath>,
    State(app_state): State<SharedState>,
    request: Request<Body>,
) -> ApiResult<Response> {
    debug!("dash get");
    let asset_id: model::AssetId = AssetId(path.id).try_into()?;
    let asset = repository::asset::get_asset_base(&app_state.pool, asset_id).await?;
    if asset.ty != AssetType::Video {
        debug!("not a video");
        return Ok(StatusCode::BAD_REQUEST.into_response());
    }
    let video_info = repository::asset::get_video_info(&app_state.pool, asset_id).await?;
    if video_info.dash_resource_dir.is_none() {
        debug!("no dash resource dir");
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let parsed_path = PathBuf::from(path.file_name)
        .file_name()
        .unwrap()
        .to_os_string();
    let dash_dir = repository::resource_file::get_resource_file_resolved(
        &app_state.pool,
        video_info.dash_resource_dir.unwrap(),
    )
    .await?;
    let full_path = dash_dir.path_on_disk().join(&parsed_path);
    let serve_file = tower_http::services::ServeFile::new(&full_path)
        .oneshot(request)
        .in_current_span()
        .await
        .unwrap()
        .into_response();
    let headers = ([
        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        (header::ACCESS_CONTROL_ALLOW_HEADERS, "*"),
        (header::ACCESS_CONTROL_ALLOW_METHODS, "*"),
    ]);
    Ok((headers, serve_file).into_response())
}
