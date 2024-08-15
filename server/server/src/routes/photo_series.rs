use core::{
    interact,
    model::{self, repository},
};

use axum::{extract::State, routing::post, Json, Router};
use core::deadpool_diesel;
use eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{AssetId, AssetSeriesId},
};

pub fn router() -> Router<SharedState> {
    Router::new().route("/", post(create_series))
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
#[tracing::instrument(fields(request = true), skip(app_state))]
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
