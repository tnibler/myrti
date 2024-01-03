use core::model::{
    self,
    repository::{self, pool::DbPool, timeline::TimelineElement},
};

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use eyre::Context;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, Instrument};
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{
        asset::{AssetSpe, AssetWithSpe, Image, ImageRepresentation, Video},
        timeline::{TimelineChunk, TimelineGroup, TimelineGroupType},
    },
};

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TimelineRequest {
    pub last_asset_id: Option<String>,
    pub max_count: i32,
    pub last_fetch: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/asset/timeline",
    params(TimelineRequest),
    responses(
    (status = 200, body=TimelineChunk)
    )
)]
#[instrument(skip(app_state))]
pub async fn get_timeline(
    State(app_state): State<SharedState>,
    Query(req_body): Query<TimelineRequest>,
) -> ApiResult<Json<TimelineChunk>> {
    debug!(?req_body);
    let local_tz = &chrono::Local; // TODO inject from config
    let now = Utc::now();
    let last_asset_id = match req_body.last_asset_id {
        Some(s) => Some(model::AssetId(s.parse().wrap_err("bad asset id")?)),
        None => None,
    };
    let groups = repository::timeline::get_timeline_chunk(
        &app_state.pool,
        last_asset_id,
        req_body.max_count.into(),
    )
    .in_current_span()
    .await?;
    let filtered_nonempty_groups = groups.into_iter().filter(|group| match group {
        TimelineElement::DayGrouped(assets) => !assets.is_empty(),
        TimelineElement::Group { group: _, assets } => !assets.is_empty(),
    });
    let mut api_groups: Vec<TimelineGroup> = Vec::default();
    for group in filtered_nonempty_groups {
        let mut api_assets_with_spe: Vec<AssetWithSpe> = Vec::default();
        let assets = match &group {
            TimelineElement::DayGrouped(assets) => assets,
            TimelineElement::Group { group: _, assets } => assets,
        };
        for asset in assets {
            api_assets_with_spe.push(asset_with_spe(&app_state.pool, asset).await?);
        }
        let api_group = match group {
            TimelineElement::DayGrouped(assets) => TimelineGroup {
                ty: TimelineGroupType::Day {
                    date: assets
                        .last()
                        .unwrap() // groups are nonempty
                        .base
                        .taken_date
                        .with_timezone(local_tz)
                        .date_naive(),
                },
                assets: api_assets_with_spe,
            },
            TimelineElement::Group { group, assets } => TimelineGroup {
                ty: TimelineGroupType::Group {
                    group_title: group.name.unwrap_or(String::from("NONAME")),
                    // unwrap is ok because empty asset vecs are filtered out above
                    group_start_date: assets.first().unwrap().base.taken_date,
                    // FIXME these should maybe not be UTC but local dates
                    group_end_date: assets.last().unwrap().base.taken_date,
                    group_id: group.id.0.to_string(),
                },
                assets: api_assets_with_spe,
            },
        };
        api_groups.push(api_group);
    }
    Ok(Json(TimelineChunk {
        date: now,
        changed_since_last_fetch: false,
        groups: api_groups,
    }))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSectionsResponse {
    pub sections: Vec<TimelineSection>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSection {
    pub id: String,
    pub num_assets: i64,
    pub avg_aspect_ratio: f32,
}

//
// pub async fn get_timeline_sections(
//     State(app_state): State<SharedState>,
// ) -> ApiResult<Json<TimelineSectionsResponse>> {
//
// }

async fn asset_with_spe(pool: &DbPool, asset: &model::Asset) -> eyre::Result<AssetWithSpe> {
    match &asset.sp {
        model::AssetSpe::Image(_image) => {
            let reprs =
                repository::representation::get_image_representations(pool, asset.base.id).await?;
            let api_reprs = reprs
                .into_iter()
                .map(|repr| ImageRepresentation {
                    id: repr.id.0.to_string(),
                    format: repr.format_name,
                    width: repr.width,
                    height: repr.height,
                    size: repr.file_size,
                })
                .collect();
            Ok(AssetWithSpe {
                asset: asset.into(),
                spe: AssetSpe::Image(Image {
                    representations: api_reprs,
                }),
            })
        }
        model::AssetSpe::Video(_video) => Ok(AssetWithSpe {
            asset: asset.into(),
            spe: AssetSpe::Video(Video {}),
        }),
    }
}
