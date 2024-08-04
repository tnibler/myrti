use core::{
    core::scheduler::{SchedulerMessage, UserRequest},
    model::{self},
};

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use serde::Deserialize;
use tracing::info;

use crate::{app_state::SharedState, http_error::HttpError};

pub mod album;
pub mod asset;
pub mod asset_roots;
pub mod dash;
pub mod jobs;
pub mod timeline;
pub mod timeline_group;

#[derive(Deserialize)]
struct QueryIndexAssetRoot {
    id: i64,
}

async fn post_index_asset_root(
    asset_root_dir_id: Query<QueryIndexAssetRoot>,
    app_state: State<SharedState>,
) -> Result<impl IntoResponse, HttpError> {
    let id = model::AssetRootDirId(asset_root_dir_id.0.id);
    info!("reindex dir {}", id);
    // let asset_root_dir = repository::asset_root_dir::get_asset_root(&app_state.pool, id).await?;
    // dbg!(&asset_root_dir);
    app_state
        .scheduler
        .send
        .send(SchedulerMessage::UserRequest(
            UserRequest::ReindexAssetRoot(id),
        ))
        .await
        .unwrap();
    Ok(())
}

pub fn api_router() -> Router<SharedState> {
    Router::new().route("/indexAssetRoot", post(post_index_asset_root))
}
