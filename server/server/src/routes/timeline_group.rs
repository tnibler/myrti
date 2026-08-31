use axum::{
    Json, Router,
    extract::State,
    routing::{patch, post},
};
use chrono::{DateTime, Utc};
use eyre::{Context, Result, eyre};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use myrti_data::{interact, model, repository};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{AssetId, TimelineGroupId},
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", post(create_timeline_group))
        .route("/", patch(edit_timeline_group))
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
    path = "/api/timelinegroups",
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

#[derive(Debug, Copy, Clone, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EditTimelineGroup {
    Add,
    Remove,
}

#[derive(Debug, Clone, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EditTimelineGroupRequest {
    pub assets: Vec<AssetId>,
    pub group_id: TimelineGroupId,
    pub operation: EditTimelineGroup,
}

#[utoipa::path(
    patch,
    path = "/api/timelinegroups",
    request_body = EditTimelineGroupRequest,
    responses((status = 200)),
)]
pub async fn edit_timeline_group(
    State(app_state): State<SharedState>,
    Json(request): Json<EditTimelineGroupRequest>,
) -> ApiResult<()> {
    if request.assets.is_empty() {
        return Err(eyre!("assetIds can not be empty").into());
    }
    let asset_ids: Vec<model::AssetId> = request
        .assets
        .into_iter()
        .map(|id| id.try_into())
        .collect::<Result<Vec<_>>>()?;
    let group_id: model::TimelineGroupId = request.group_id.try_into()?;
    let conn = app_state.pool.get().await?;
    interact!(conn, move |conn| {
        match request.operation {
            EditTimelineGroup::Add => {
                repository::timeline_group::add_assets_to_group(conn, group_id, &asset_ids)
            }
            EditTimelineGroup::Remove => {
                repository::timeline_group::remove_assets_from_group(conn, group_id, &asset_ids)
            }
        }
    })
    .await??;
    Ok(())
}
