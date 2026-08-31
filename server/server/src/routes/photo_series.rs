use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, patch, post},
};
use eyre::{Context, Result, eyre};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use myrti_data::{interact, model, repository};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{AssetId, AssetSeriesId, asset_series::AssetSeries},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_series))
        .route("/:id", patch(add_assets_to_series))
        .route("/:id", delete(delete_series))
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesRequest {
    pub asset_ids: Vec<AssetId>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSeriesResponse {
    pub series_id: AssetSeriesId,
}

#[utoipa::path(
    post,
    path = "/api/photoSeries",
    responses((status=200, body=CreateSeriesResponse))
)]
#[tracing::instrument(fields(request = true), skip(app_state), err)]
pub async fn create_series(
    State(app_state): State<SharedState>,
    Json(request): Json<CreateSeriesRequest>,
) -> ApiResult<Json<CreateSeriesResponse>> {
    if request.asset_ids.is_empty() {
        return Err(eyre!("assetIds can not be empty").into());
    }
    let asset_ids: Vec<model::AssetId> = request
        .asset_ids
        .iter()
        .map(model::AssetId::try_from)
        .collect::<Result<Vec<_>>>()
        .wrap_err("invalid assetIds")?;
    let conn = app_state.pool.get().await?;
    let series_id = interact!(conn, move |conn| {
        repository::asset_series::create_series(conn, &asset_ids)
    })
    .await
    .wrap_err("error creating AssetSeries")??;
    Ok(Json(CreateSeriesResponse {
        series_id: AssetSeriesId(series_id.0.to_string()),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/photoSeries/:id",
    params(
        ("id" = String, Path, description = "AssetSeriesId"),
    ),
    responses((status=200, body=())),
)]
#[tracing::instrument(fields(request = true), skip(app_state), err)]
pub async fn delete_series(
    State(app_state): State<SharedState>,
    Path(series_id): Path<AssetSeriesId>,
) -> ApiResult<()> {
    let series_id = model::AssetSeriesId::try_from(series_id).wrap_err("invalid AssetSeriesId")?;
    let conn = app_state.pool.get().await?;
    interact!(conn, move |conn| {
        repository::asset_series::dissolve_series(conn, series_id)
    })
    .await
    .wrap_err("error deleting AssetSeries")??;
    Ok(())
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddAssetsToSeriesRequest {
    pub asset_ids: Vec<AssetId>,
}

#[utoipa::path(
    patch,
    path = "/api/photoSeries/:id",
    params(
        ("id" = String, Path, description = "AssetSeriesId"),
    ),
    responses((status=200, body=AssetSeries)),
)]
#[tracing::instrument(fields(request = true), skip(app_state), err)]
pub async fn add_assets_to_series(
    State(app_state): State<SharedState>,
    Path(series_id): Path<AssetSeriesId>,
    Json(request): Json<AddAssetsToSeriesRequest>,
) -> ApiResult<Json<AssetSeries>> {
    if request.asset_ids.is_empty() {
        return Err(eyre!("assetIds can not be empty").into());
    }
    let asset_ids: Vec<model::AssetId> = request
        .asset_ids
        .iter()
        .map(model::AssetId::try_from)
        .collect::<Result<Vec<_>>>()
        .wrap_err("invalid assetIds")?;
    let series_id = model::AssetSeriesId::try_from(series_id).wrap_err("invalid AssetSeriesId")?;
    let conn = app_state.pool.get().await?;
    let series = interact!(conn, move |conn| {
        repository::asset_series::add_assets_to_series(conn, series_id, &asset_ids)
    })
    .await
    .wrap_err("error updating AssetSeries")??;
    Ok(Json(AssetSeries::from_model(&series)))
}
