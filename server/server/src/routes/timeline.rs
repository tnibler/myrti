use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, Instrument};
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{
        asset::{Asset, AssetSpe, AssetWithSpe, Image, ImageRepresentation, Video},
        timeline::{TimelineChunk, TimelineGroup, TimelineGroupType},
        TimelineGroupId,
    },
};
use core::{
    deadpool_diesel, interact,
    model::{
        self,
        repository::{
            self,
            db::DbPool,
            timeline::{TimelineElement, TimelineSegmentType},
        },
    },
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/sections", get(get_timeline_sections))
        .route("/segments", get(get_timeline_segments))
}

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
#[instrument(skip(app_state), level = "trace")]
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
    let conn = app_state.pool.get().in_current_span().await?;
    let groups = interact!(conn, move |mut conn| {
        repository::timeline::get_timeline_chunk(
            &mut conn,
            last_asset_id,
            req_body.max_count.into(),
        )
    })
    .in_current_span()
    .await??;
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

#[utoipa::path(
    get,
    path = "/api/timeline/sections",
    responses(
    (status = 200, body=TimelineSectionsResponse)
    )
)]
#[instrument(skip(app_state), level = "trace")]
pub async fn get_timeline_sections(
    State(app_state): State<SharedState>,
) -> ApiResult<Json<TimelineSectionsResponse>> {
    let conn = app_state.pool.get().in_current_span().await?;
    let sections: Vec<TimelineSection> = interact!(conn, move |mut conn| {
        repository::timeline::get_sections(&mut conn)
    })
    .in_current_span()
    .await??
    .into_iter()
    .map(|section| TimelineSection {
        id: format!("{}_{}", section.id.segment_min, section.id.segment_max),
        num_assets: section.num_assets,
        avg_aspect_ratio: 3.0 / 2.0,
    })
    .collect();
    Ok(Json(TimelineSectionsResponse { sections }))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SegmentType {
    DateRange {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
    UserGroup {
        id: TimelineGroupId,
        name: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSegment {
    #[serde(rename = "segment")]
    pub segment: SegmentType,
    pub assets: Vec<AssetWithSpe>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSegmentsRequest {
    pub section_id: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TimelineSegmentsResponse {
    pub segments: Vec<TimelineSegment>,
}

#[utoipa::path(
    get,
    path = "/api/timeline/segments",
    params(TimelineSegmentsRequest),
    responses(
    (status = 200, body=TimelineSegmentsResponse)
    )
)]
#[instrument(skip(app_state), level = "trace")]
pub async fn get_timeline_segments(
    Query(query): Query<TimelineSegmentsRequest>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<TimelineSegmentsResponse>> {
    let (segment_min, segment_max) = query
        .section_id
        .split_once("_")
        .ok_or(eyre!("invalid sectionId"))?;
    let segment_min: i64 = segment_min.parse().wrap_err("invalid sectionId")?;
    let segment_max: i64 = segment_max.parse().wrap_err("invalid sectionId")?;
    let conn = app_state.pool.get().in_current_span().await?;
    let segments = interact!(conn, move |mut conn| {
        repository::timeline::get_segments_in_section(&mut conn, segment_min, segment_max)
    })
    .in_current_span()
    .await??
    .into_iter()
    .map(|segment| TimelineSegment {
        assets: segment
            .assets
            .into_iter()
            .map(|asset| asset.into())
            .collect(),
        segment: match segment.ty {
            TimelineSegmentType::Group(repository::timeline::TimelineGroupType::UserCreated(
                group,
            )) => SegmentType::UserGroup {
                id: TimelineGroupId(group.id.to_string()),
                name: group.name,
            },
            TimelineSegmentType::DateRange { start, end } => SegmentType::DateRange { start, end },
        },
    })
    .collect();
    Ok(Json(TimelineSegmentsResponse { segments }))
}

async fn asset_with_spe(pool: &DbPool, asset: &model::Asset) -> eyre::Result<AssetWithSpe> {
    let conn = pool.get().in_current_span().await?;
    match &asset.sp {
        model::AssetSpe::Image(_image) => {
            let asset_id = asset.base.id;
            let reprs = interact!(conn, move |mut conn| {
                repository::representation::get_image_representations(&mut conn, asset_id)
            })
            .in_current_span()
            .await??;
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
        model::AssetSpe::Video(video) => Ok(AssetWithSpe {
            asset: asset.into(),
            spe: AssetSpe::Video(Video {
                has_dash: video.has_dash,
            }),
        }),
    }
}
