use core::{
    deadpool_diesel, interact,
    model::{self, repository},
};

use axum::{extract::State, routing::post, Json, Router};
use chrono::{DateTime, Utc};
use eyre::{eyre, Context, Result};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{AssetId, TimelineGroupId},
};

pub fn router() -> Router<SharedState> {
    Router::new().route("/", post(create_timeline_group))
}

#[derive(Debug, Clone, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateTimelineGroupRequest {
    pub assets: Vec<AssetId>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateTimelineGroupResponse {
    pub timeline_group_id: TimelineGroupId,
    pub display_date: DateTime<Utc>,
}

#[utoipa::path(
    post,
    path = "/api/timelinegroup",
    request_body = CreateTimelineGroupRequest,
    responses((status = 200, body=CreateTimelineGroupResponse)),
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn create_timeline_group(
    State(app_state): State<SharedState>,
    Json(request): Json<CreateTimelineGroupRequest>,
) -> ApiResult<Json<CreateTimelineGroupResponse>> {
    if request.assets.is_empty() {
        return Err(eyre!("assetIds can not be empty").into());
    }
    if request.name.is_empty() {
        return Err(eyre!("name can not be empty").into());
    }
    let asset_ids: Vec<model::AssetId> = request
        .assets
        .into_iter()
        .map(|id| id.try_into())
        .collect::<Result<Vec<_>>>()?;
    let conn = app_state.pool.get().await?;
    let asset_ids_copy = asset_ids.clone();
    let display_date = interact!(conn, move |conn| {
        repository::timeline_group::get_newest_asset_date(conn, &asset_ids_copy)
    })
    .await?
    .wrap_err("could not get display date to create TimelineGroup")?
    .ok_or(eyre!("could not get display date to create TimelineGroup"))?;
    let create_timeline_group = repository::timeline_group::CreateTimelineGroup {
        name: Some(request.name),
        display_date,
        asset_ids,
    };
    let timeline_group_id = interact!(conn, move |conn| {
        repository::timeline_group::create_timeline_group(conn, create_timeline_group)
    })
    .await?
    .wrap_err("error creating TimelineGroup")?;
    Ok(Json(CreateTimelineGroupResponse {
        timeline_group_id: timeline_group_id.into(),
        display_date,
    }))
}
