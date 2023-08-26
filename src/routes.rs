use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use eyre::Context;
use serde::Deserialize;
use tracing::info;

use crate::{
    app_state::SharedState,
    core::scheduler::{SchedulerMessage, UserRequest},
    eyre::Result,
    http_error::HttpError,
    job::indexing_job::IndexingJobParams,
    model::repository,
    model::AssetRootDirId,
};

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
    let asset_root = repository::asset_root_dir::get_asset_root(&app_state.pool, id)
        .await
        .wrap_err("No asset root with this id")?;
    let params = IndexingJobParams {
        asset_root,
        sub_paths: None,
    };
    app_state
        .scheduler
        .send(SchedulerMessage::UserRequest(
            UserRequest::ReindexAssetRoots { params },
        ))
        .await
        .unwrap();
    Ok(())
}

pub fn api_router() -> Router<SharedState> {
    Router::new().route("/indexAssetRoot", post(post_index_asset_root))
}
