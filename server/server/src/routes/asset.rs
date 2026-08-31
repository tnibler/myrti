use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use eyre::{Context, Result, eyre};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use myrti_data::{interact, model, repository};

use crate::{
    app_state::SharedState,
    http_error::{ApiResult, HttpErrorExt},
    schema::{AssetId, AssetSeriesId, asset::Asset},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/:id", get(get_asset))
        .route("/hidden", post(set_assets_hidden))
        .route("/:id/seriesSelection", post(set_asset_is_series_selection))
}

#[utoipa::path(get, path = "/api/assets/{id}",
    responses(
        (status = 200, body = Asset),
        (status = NOT_FOUND, description = "Asset not found")
    ),
    params(
        ("id" = String, Path, description = "AssetId")
    )
)]
async fn get_asset(
    Path(_id): Path<String>,
    State(_app_state): State<SharedState>,
) -> ApiResult<Json<Asset>> {
    Err(eyre!("not implemented"))?
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum HideAssetAction {
    Hide,
    Unhide,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HideAssetsRequest {
    pub what: HideAssetAction,
    pub asset_ids: Vec<AssetId>,
}

#[utoipa::path(
    post,
    path = "/api/assets/hidden",
    request_body=HideAssetsRequest,
    responses((status=200)),
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
async fn set_assets_hidden(
    State(app_state): State<SharedState>,
    Json(req): Json<HideAssetsRequest>,
) -> ApiResult<()> {
    let asset_ids: Vec<model::AssetId> = req
        .asset_ids
        .into_iter()
        .map(model::AssetId::try_from)
        .collect::<Result<Vec<_>>>()?;
    let conn = app_state.pool.get().await?;
    interact!(conn, move |conn| {
        repository::asset::set_assets_hidden(conn, true, &asset_ids)
    })
    .await?
    .wrap_err("error setting Assets hidden")?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetAssetSeriesSelectionRequest {
    pub is_series_selection: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetAssetIsSeriesSelectionResponse {
    series_id: AssetSeriesId,
    asset_ids: Vec<AssetId>,
    selection_indices: Vec<usize>,
}

#[utoipa::path(
    post,
    path = "/api/assets/{id}/seriesSelection",
    params(
        ("id" = String, Path, description = "AssetId")
    ),
    request_body=SetAssetSeriesSelectionRequest,
    responses(
        (status = 200, body=SetAssetIsSeriesSelectionResponse)
    ),
)]
async fn set_asset_is_series_selection(
    State(app_state): State<SharedState>,
    Path(asset_id): Path<AssetId>,
    Json(req): Json<SetAssetSeriesSelectionRequest>,
) -> ApiResult<Json<SetAssetIsSeriesSelectionResponse>> {
    let asset_id: model::AssetId = asset_id.try_into()?;
    let conn = app_state.pool.get().await?;
    let series = interact!(conn, move |conn| {
        repository::asset::set_asset_is_series_selection(conn, asset_id, req.is_series_selection)?;
        repository::asset_series::get_series_for_asset(conn, asset_id)
    })
    .await??;
    let series = series.ok_or(eyre!("Asset is not part of a series").into_400())?;
    Ok(Json(SetAssetIsSeriesSelectionResponse {
        series_id: series.series_id.into(),
        asset_ids: series.asset_ids.into_iter().map(AssetId::from).collect(),
        selection_indices: series.selection_indices,
    }))
}
