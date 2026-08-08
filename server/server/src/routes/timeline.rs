use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use eyre::Context;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::SharedState,
    asset_queries::get_full_asset,
    http_error::{ApiResult, HttpErrorExt},
    schema::{AssetSeriesId, TimelineGroupId, TimelineSectionId, asset::AssetWithSpe},
};
use myrti_core::{
    deadpool_diesel, interact,
    model::{
        self,
        repository::{
            self,
            timeline::{AssetsInTimeline, TimelineSegmentType},
        },
    },
};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/rebuild", post(rebuild_timeline))
        .route("/sections", get(get_timeline_sections))
        .route("/sections/:id", get(get_timeline_segments))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSectionsResponse {
    pub sections: Vec<TimelineSection>,
    pub months_summary: Vec<Vec<TimelineMonthSlice>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSection {
    pub id: TimelineSectionId,
    pub num_assets: i32,
    pub total_normalized_width: f64,
    /// date of *most recent* asset in range
    pub start_date: DateTime<Utc>,
    /// date of *oldest* asset in range
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineMonthSlice {
    pub year: i32,
    pub month: i32,
    pub num_assets: i32,
    pub total_normalized_width: f64,
}

#[utoipa::path(
    post,
    path = "/api/timeline/rebuild",
    responses(
        (status = 200)
    )
)]
#[instrument(skip(app_state))]
pub async fn rebuild_timeline(State(app_state): State<SharedState>) -> ApiResult<()> {
    let conn = app_state.pool.get().await?;
    interact!(conn, move |conn| {
        repository::timeline::rebuild_timeline_full(conn)
    })
    .await??;
    Ok(())
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
        id: TimelineSectionId(section.id.0.to_string()),
        num_assets: section.num_assets,
        total_normalized_width: section.total_normalized_width,
        start_date: section.start_date,
        end_date: section.end_date,
    })
    .collect();
    let section_months: Vec<Vec<TimelineMonthSlice>> =
        interact!(conn, move |conn| { repository::timeline::get_months(conn) })
            .await??
            .into_iter()
            .map(|month| {
                (
                    month.section_idx,
                    TimelineMonthSlice {
                        year: month.year,
                        month: month.month,
                        num_assets: month.num_assets,
                        total_normalized_width: month.total_normalized_width,
                    },
                )
            })
            .fold(
                (-1, Vec::new()),
                |(last_section_idx, mut acc): (i64, Vec<Vec<TimelineMonthSlice>>),
                 (section_idx, month_slice): (i64, TimelineMonthSlice)| {
                    if let Some(last) = acc.last_mut()
                        && last_section_idx == section_idx
                    {
                        last.push(month_slice);
                    } else {
                        acc.push(Vec::from([month_slice]));
                    }
                    (section_idx, acc)
                },
            )
            .1;
    Ok(Json(TimelineSectionsResponse {
        sections,
        months_summary: section_months,
    }))
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
    pub items: Vec<TimelineItem>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", tag = "itemType")]
pub enum TimelineItem {
    Asset(AssetWithSpe),
    #[serde(rename_all = "camelCase")]
    AssetSeries {
        series_id: AssetSeriesId,
        /// assets[0] is most recent, last is oldest asset
        assets: Vec<AssetWithSpe>,
        selection_indices: Vec<usize>,
        total_size: usize,
    },
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
    ),
    params(
        ("id"=String, description="Section id")
    )
)]
#[tracing::instrument(fields(request = true), skip(app_state))]
pub async fn get_timeline_segments(
    Path(section_id): Path<TimelineSectionId>,
    State(app_state): State<SharedState>,
) -> ApiResult<Json<TimelineSegmentsResponse>> {
    let section_id: model::TimelineSectionId =
        section_id.try_into().wrap_err("invalid sectionId")?;
    let mut conn = app_state.pool.get().await?;
    let segments = interact!(conn, move |conn| {
        repository::timeline::get_segments_in_section(conn, section_id)
    })
    .await??;
    let mut result: Vec<TimelineSegment> = Vec::default();
    for segment in segments {
        let mut timeline_items = Vec::default();
        for item in segment.items {
            match item {
                AssetsInTimeline::Asset(asset) => {
                    let asset_with_reprs = get_full_asset(&mut conn, asset).await?;
                    timeline_items.push(TimelineItem::Asset(asset_with_reprs));
                }
                AssetsInTimeline::AssetSeries {
                    assets,
                    series_id,
                    series_date: _,
                    selection_indices,
                    total_series_size,
                } => {
                    let mut assets_with_reprs: Vec<AssetWithSpe> = Vec::default();
                    for asset in assets {
                        assets_with_reprs.push(get_full_asset(&mut conn, asset).await?);
                    }
                    timeline_items.push(TimelineItem::AssetSeries {
                        series_id: series_id.into(),
                        assets: assets_with_reprs,
                        selection_indices,
                        total_size: total_series_size,
                    });
                }
            }
        }
        result.push(TimelineSegment {
            items: timeline_items,
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
        });
    }
    Ok(Json(TimelineSegmentsResponse { segments: result }))
}
