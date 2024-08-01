use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use eyre::{eyre, Context, Result};
use futures::{stream::FuturesOrdered, TryStreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::SharedState,
    http_error::ApiResult,
    schema::{
        asset::{AssetSpe, AssetWithSpe, Image, ImageRepresentation, Video},
        timeline::{TimelineChunk, TimelineGroup, TimelineGroupType},
        AssetId, TimelineGroupId,
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
        .route("/sections/:id", get(get_timeline_segments))
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TimelineRequest {
    pub last_asset_id: Option<AssetId>,
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
    let last_asset_id: Option<model::AssetId> = req_body
        .last_asset_id
        .map(model::AssetId::try_from)
        .transpose()?;
    let conn = app_state.pool.get().await?;
    let groups = interact!(conn, move |conn| {
        repository::timeline::get_timeline_chunk(conn, last_asset_id, req_body.max_count.into())
    })
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
    /// date of *most recent* asset in range
    pub start_date: DateTime<Utc>,
    /// date of *oldest* asset in range
    pub end_date: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/timeline/sections",
    responses(
    (status = 200, body=TimelineSectionsResponse)
    )
)]
#[instrument(skip(app_state))]
pub async fn get_timeline_sections(
    State(app_state): State<SharedState>,
) -> ApiResult<Json<TimelineSectionsResponse>> {
    let conn = app_state.pool.get().await?;
    let sections: Vec<TimelineSection> = interact!(conn, move |conn| {
        repository::timeline::get_sections(conn)
    })
    .await??
    .into_iter()
    .map(|section| TimelineSection {
        id: format!("{}_{}", section.id.segment_min, section.id.segment_max),
        num_assets: section.num_assets,
        avg_aspect_ratio: 3.0 / 2.0,
        start_date: section.start_date,
        end_date: section.end_date,
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
    #[serde(flatten)]
    pub segment: SegmentType,
    pub sort_date: DateTime<Utc>,
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
    path = "/api/timeline/sections/{id}",
    responses(
    (status = 200, body=TimelineSegmentsResponse)
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn get_timeline_segments(
    Path(section_id): Path<String>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<TimelineSegmentsResponse>> {
    let (segment_min, segment_max) = section_id
        .split_once('_')
        .ok_or(eyre!("invalid sectionId"))?;
    let segment_min: i64 = segment_min.parse().wrap_err("invalid sectionId")?;
    let segment_max: i64 = segment_max.parse().wrap_err("invalid sectionId")?;
    let conn = app_state.pool.get().await?;
    let pool = &app_state.pool;
    let segments_result: Result<Vec<TimelineSegment>> = interact!(conn, move |conn| {
        repository::timeline::get_segments_in_section(conn, segment_min, segment_max)
    })
    .await??
    .into_iter()
    .map(|segment| async move {
        // convert model::Asset to schema::AssetWithSpe, populating the representations field
        // with the list of available image representions if the original format is not in a
        // (hardcoded) list of acceptable formats.
        // This does some async stream stuff which is probably unnecessary and could just as
        // well run sequentially, there was no thought put into performace at the time of writing.
        let assets_with_reprs =
            assets_with_alt_reprs_if_required(pool.clone(), segment.assets).await?;
        Ok(TimelineSegment {
            assets: assets_with_reprs,
            sort_date: segment.sort_date,
            segment: match segment.ty {
                TimelineSegmentType::Group(
                    repository::timeline::TimelineGroupType::UserCreated(group),
                ) => SegmentType::UserGroup {
                    id: TimelineGroupId(group.id.0.to_string()),
                    name: group.name,
                },
                TimelineSegmentType::DateRange { start, end } => {
                    SegmentType::DateRange { start, end }
                }
            },
        })
    })
    .collect::<FuturesOrdered<_>>()
    .try_collect::<Vec<_>>()
    .await;
    let segments = segments_result?;

    Ok(Json(TimelineSegmentsResponse { segments }))
}

#[tracing::instrument(skip(pool))]
async fn asset_with_reprs(pool: DbPool, asset: model::Asset) -> eyre::Result<AssetWithSpe> {
    let spe: AssetSpe = match &asset.sp {
        model::AssetSpe::Image(image) => {
            let reprs = match image.image_format_name.as_str() {
                // TODO hardcoded list of client accepted image formats
                "jpeg" | "avif" | "png" => Vec::new(),
                _ => {
                    let conn = pool.get().await?;
                    interact!(conn, move |conn| {
                        repository::representation::get_image_representations(conn, asset.base.id)
                    })
                    .await??
                }
            };
            AssetSpe::Image(Image {
                representations: reprs
                    .into_iter()
                    .map(|repr| ImageRepresentation {
                        id: repr.id.0.to_string(),
                        format: repr.format_name,
                        width: repr.width,
                        height: repr.height,
                        size: repr.file_size,
                    })
                    .collect(),
            })
        }
        model::AssetSpe::Video(video) => AssetSpe::Video(Video {
            has_dash: video.has_dash,
        }),
    };
    Ok(AssetWithSpe {
        asset: asset.into(),
        spe,
    })
}

#[tracing::instrument(skip(pool))]
async fn assets_with_alt_reprs_if_required(
    pool: DbPool,
    assets: Vec<model::Asset>,
) -> Result<Vec<AssetWithSpe>> {
    assets
        .into_iter()
        .map(|asset| async { asset_with_reprs(pool.clone(), asset).await })
        .collect::<FuturesOrdered<_>>()
        .try_collect::<Vec<_>>()
        .await
}

async fn asset_with_spe(pool: &DbPool, asset: &model::Asset) -> eyre::Result<AssetWithSpe> {
    let conn = pool.get().await?;
    match &asset.sp {
        model::AssetSpe::Image(_image) => {
            let asset_id = asset.base.id;
            let reprs = interact!(conn, move |conn| {
                repository::representation::get_image_representations(conn, asset_id)
            })
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
