use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use tracing::Instrument;

use core::model::{self, repository};
use core::{deadpool_diesel, interact};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{AssetRoot, AssetRootDirId},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_asset_roots))
        .route("/:id", get(get_asset_root_by_id))
}

async fn get_asset_roots(app_state: State<SharedState>) -> ApiResult<Json<Vec<AssetRoot>>> {
    let conn = app_state.pool.get().await?;
    let asset_roots = interact!(conn, move |mut conn| {
        repository::asset_root_dir::get_asset_roots(&mut conn)
    })
    .await??;
    Ok(asset_roots
        .into_iter()
        .map(|model| AssetRoot {
            id: model.id.into(),
            path: model.path,
            num_assets: 0,
            last_full_reindex: None,
            last_change: None,
        })
        .collect::<Vec<_>>()
        .into())
}

async fn get_asset_root_by_id(
    Path(path_id): Path<String>,
    app_state: State<SharedState>,
) -> ApiResult<Json<AssetRoot>> {
    let id: model::AssetRootDirId = AssetRootDirId(path_id).try_into()?;
    let conn = app_state.pool.get().await?;
    let model = interact!(conn, move |mut conn| {
        repository::asset_root_dir::get_asset_root(&mut conn, id)
    })
    .await??;
    Ok(AssetRoot {
        id: model.id.into(),
        path: model.path,
        // TODO fill out these fields
        num_assets: 0,
        last_full_reindex: None,
        last_change: None,
    }
    .into())
}
