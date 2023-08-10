use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{uri::PathAndQuery, Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, options},
    Router,
};
use eyre::{bail, eyre, Context};
use serde::Deserialize;
use tower::{service_fn, MakeService, Service, ServiceExt};
use tower_http::services::ServeDir;
use tracing::{debug, instrument, Instrument};

use crate::{
    api::{schema::AssetId, ApiResult},
    app_state::SharedState,
    model::{self, repository, AssetType},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        // .nest_service("/:id", get(serve_dash_dir))
        // .nest_service("/:id", options(serve_dash_dir))
        .route("/:id/*path", get(get_dash_file))
        .route("/:id/*path", options(get_dash_file))
}

#[derive(Debug, Clone, Deserialize)]
struct DashFilePath {
    pub id: String,
    pub path: String,
}

#[instrument(skip(app_state))]
async fn get_dash_file(
    Path(path): Path<DashFilePath>,
    State(app_state): State<SharedState>,
    request: Request<Body>,
) -> ApiResult<Response> {
    let asset_id: model::AssetId = AssetId(path.id).try_into()?;
    let asset = repository::asset::get_asset_base(&app_state.pool, asset_id).await?;
    if asset.ty != AssetType::Video {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    }
    let video_info = repository::asset::get_video_info(&app_state.pool, asset_id).await?;
    if video_info.dash_resource_dir.is_none() {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let dash_dir = repository::resource_file::get_resource_file_resolved(
        &app_state.pool,
        video_info.dash_resource_dir.unwrap(),
    )
    .await?;
    let (mut parts, body) = request.into_parts();
    let mut uri_parts = parts.uri.clone().into_parts();
    uri_parts.path_and_query = Some(path.path.as_str().try_into().wrap_err("bad path")?);
    parts.uri = Uri::from_parts(uri_parts).wrap_err("bad uri")?;
    let serve_dir = tower_http::services::ServeDir::new(dash_dir.path_on_disk())
        .oneshot(Request::from_parts(parts, body))
        .in_current_span()
        .await
        .wrap_err("error serving file")?;
    Ok(serve_dir.into_response())
}
