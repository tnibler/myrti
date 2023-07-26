use crate::{
    app_state::SharedState,
    eyre::Result,
    http_error::HttpError,
    model::AssetRootDirId,
    repository::{self, asset_root_dir},
    scheduler::{SchedulerEvent, UserRequest},
};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use eyre::eyre;
use serde::Deserialize;
use tracing::info;

async fn get_assets(app_state: State<SharedState>) -> Result<impl IntoResponse, HttpError> {
    repository::asset::get_assets(&app_state.pool)
        .await
        .map_err(|e| e.into())
        .map(|v| Json(v))
}

async fn get_asset_roots(app_state: State<SharedState>) -> Result<impl IntoResponse, HttpError> {
    let values = repository::asset_root_dir::get_asset_roots(&app_state.pool).await?;
    Ok(Json(values))
}

#[derive(Deserialize)]
struct QueryIndexAssetRoot {
    id: i64,
}

async fn post_index_asset_root(
    asset_root_dir_id: Query<QueryIndexAssetRoot>,
    app_state: State<SharedState>,
) -> Result<impl IntoResponse, HttpError> {
    let id = AssetRootDirId(asset_root_dir_id.0.id);
    info!("reindex dir {}", id);
    // let asset_root_dir = repository::asset_root_dir::get_asset_root(&app_state.pool, id).await?;
    // dbg!(&asset_root_dir);
    app_state
        .scheduler
        .send(SchedulerEvent::UserRequest(UserRequest::ReindexFullRoot {
            id,
        }))
        .await
        .unwrap();
    Ok(())
}

pub fn api_router() -> Router<SharedState> {
    Router::new()
        .route("/assets", get(get_assets))
        .route("/assetRoots", get(get_asset_roots))
        .route("/indexAssetRoot", post(post_index_asset_root))
}
