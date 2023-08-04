use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};


use crate::{
    api::{
        schema::{AssetRoot, AssetRootId},
        ApiResult,
    },
    app_state::SharedState,
    model::repository,
    model::AssetRootDirId,
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(get_asset_roots))
        .route("/:id", get(get_asset_root_by_id))
}

async fn get_asset_roots(app_state: State<SharedState>) -> ApiResult<Json<Vec<AssetRoot>>> {
    let asset_roots = repository::asset_root_dir::get_asset_roots(&app_state.pool).await?;
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
    let id: AssetRootDirId = AssetRootId(path_id).try_into()?;
    let model = repository::asset_root_dir::get_asset_root(&app_state.pool, id).await?;
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
