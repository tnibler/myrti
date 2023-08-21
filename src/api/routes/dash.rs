use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use eyre::Context;
use serde::Deserialize;
use tower::ServiceExt;
use tracing::{instrument, Instrument};

use crate::{
    api::{schema::AssetId, ApiResult},
    app_state::SharedState,
    model::{self, repository, VideoAsset},
};

pub fn router() -> Router<SharedState> {
    Router::new().route("/:id/*path", get(get_dash_file).options(get_dash_file))
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
    let asset = repository::asset::get_asset(&app_state.pool, asset_id).await?;
    let asset: VideoAsset = match asset.try_into() {
        Ok(v) => v,
        Err(_) => {
            return Ok(StatusCode::BAD_REQUEST.into_response());
        }
    };
    let dash_dir = match asset.video.dash_resource_dir {
        Some(p) => p,
        None => {
            return Ok(StatusCode::NO_CONTENT.into_response());
        }
    };
    let (mut parts, body) = request.into_parts();
    let mut uri_parts = parts.uri.clone().into_parts();
    uri_parts.path_and_query = Some(path.path.as_str().try_into().wrap_err("bad path")?);
    parts.uri = Uri::from_parts(uri_parts).wrap_err("bad uri")?;
    let serve_dir = tower_http::services::ServeDir::new(&dash_dir)
        .oneshot(Request::from_parts(parts, body))
        .in_current_span()
        .await
        .wrap_err("error serving file")?;
    Ok(serve_dir.into_response())
}
