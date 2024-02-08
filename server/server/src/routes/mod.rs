use core::{
    core::scheduler::{SchedulerMessage, UserRequest},
    deadpool_diesel, interact,
    job::indexing_job::IndexingJobParams,
    model::{self, repository},
};

use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use eyre::Context;
use serde::Deserialize;
use tracing::{info, Instrument};

use crate::{app_state::SharedState, http_error::HttpError};

pub mod album;
pub mod asset;
pub mod asset_roots;
pub mod dash;
pub mod jobs;
pub mod timeline;

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
    let conn = app_state.pool.get().in_current_span().await?;
    let asset_root = interact!(conn, move |mut conn| {
        repository::asset_root_dir::get_asset_root(&mut conn, id)
    })
    .in_current_span()
    .await?
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
