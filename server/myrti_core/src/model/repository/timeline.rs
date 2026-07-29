use chrono::{DateTime, Utc};
use diesel::{
    RunQueryDsl, SelectableHelper,
    connection::SimpleConnection,
    deserialize::QueryableByName,
    query_builder::{QueryBuilder, QueryFragment},
    sql_query,
    sqlite::SqliteQueryBuilder,
};
use eyre::{Context, Result, eyre};
use itertools::Itertools;
use tokio::time::Instant;
use tracing::instrument;

use crate::model::{
    Asset, AssetBase, AssetId, AssetSeriesId, TimelineGroup, TimelineGroupId,
    repository::{self, db_entity::from_db_asset_ty},
    util::datetime_from_db_repr,
};

use super::{db::DbConn, db_entity::DbAsset, timeline_group::get_timeline_group};

#[tracing::instrument(skip(conn), level = "debug")]
pub fn rebuild_timeline(conn: &mut DbConn) -> Result<()> {
    conn.immediate_transaction(|conn| {
        conn.batch_execute(include_str!("timeline_query.sql"))
            .wrap_err("error executing rebuild_timeline query")?;
        Ok(())
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineElement {
    DayGrouped(Vec<Asset>),
    Group {
        group: TimelineGroup,
        assets: Vec<Asset>,
    },
}

impl TimelineElement {
    pub fn get_assets(&self) -> &[Asset] {
        match self {
            TimelineElement::DayGrouped(assets) => assets,
            TimelineElement::Group { group: _, assets } => assets,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineSectionId {
    /// (inlusive): index of first segment in this section
    pub segment_min: i64,
    /// (inlusive): index of last segment in this section
    pub segment_max: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineSection {
    pub id: TimelineSectionId,
    pub num_assets: i64,
    /// date of *most recent* asset in section's segments
    pub start_date: DateTime<Utc>,
    /// date of *oldest* asset in section's segments
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Clone, QueryableByName)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct RowTimelineSection {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub section_idx: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub start_segment: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub end_segment: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub section_len: i64,
}

#[tracing::instrument(skip(conn), level = "debug")]
pub fn get_sections(conn: &mut DbConn) -> Result<Vec<TimelineSection>> {
    const QUERY: &str = r#"
    SELECT section_idx, start_segment, end_segment, section_len FROM TimelineSection;
    "#;
    let query_start = Instant::now();
    let rows: Vec<RowTimelineSection> = sql_query(QUERY).load(conn)?;
    let query_elapsed = query_start.elapsed();
    let sections = rows
        .into_iter()
        .map(|row| {
            Ok(TimelineSection {
                id: TimelineSectionId {
                    segment_min: row.start_segment,
                    segment_max: row.end_segment - 1, // FIXME: decide on exclusive or inclusive
                },
                start_date: datetime_from_db_repr(0)?,
                end_date: datetime_from_db_repr(10)?,
                num_assets: row.section_len,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    tracing::debug!(?query_elapsed);
    Ok(sections)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineGroupType {
    UserCreated(TimelineGroup),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineSegmentType {
    Group(TimelineGroupType),
    DateRange {
        /// date of *most recent* asset in range
        start: DateTime<Utc>,
        /// date of *oldest* asset in range
        end: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineSegment {
    pub ty: TimelineSegmentType,
    pub sort_date: DateTime<Utc>,
    pub items: Vec<AssetsInTimeline>,
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetsInTimeline {
    Asset(Asset),
    AssetSeries {
        assets: Vec<Asset>,
        series_id: AssetSeriesId,
        series_date: DateTime<Utc>,
        selection_indices: Vec<usize>,
        /// Total size of the series, not always equal to `assets.len()`.
        /// AssetSeries can theoretically be split up in the timeline, for instance if some
        /// but not all Assets in it are part of a TimelineGroup.
        total_series_size: usize,
    },
}

#[derive(Debug, Clone, QueryableByName)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct RowTimelineSegmentInSection {
    #[diesel(embed)]
    pub asset: DbAsset,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    pub timeline_group_id: Option<i64>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    pub series_id: Option<i64>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    pub series_date: Option<i64>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    pub series_len: Option<i32>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    pub is_series_selection: Option<i32>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub sort_date: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub segment_idx: i64,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    pub segment_split_idx: Option<i32>,
}

#[instrument(err(Debug), skip(conn), level = "debug")]
pub fn get_segments_in_section(
    conn: &mut DbConn,
    segment_min: i64,
    segment_max: i64,
) -> Result<Vec<TimelineSegment>> {
    let mut qb = SqliteQueryBuilder::new();
    qb.push_sql(
        r#"
    SELECT
    "#,
    );
    DbAsset::as_select().to_sql(&mut qb, &diesel::sqlite::Sqlite)?;
    qb.push_sql(
        r#"
   , TimelineItem.group_id as timeline_group_id
    , TimelineItem.series_id as series_id
    , TimelineItem.series_date as series_date
    , CASE WHEN TimelineItem.series_id IS NULL THEN NULL ELSE 1 END as series_len
    , Asset.is_series_selection as is_series_selection
    , TimelineItem.sort_date as sort_date
    , TimelineItem.segment_idx as segment_idx
    , TimelineItem.segment_split_idx as segment_split_idx
    FROM
    TimelineItem INNER JOIN Asset ON Asset.asset_id = TimelineItem.asset_id
    WHERE
    ? <= TimelineItem.segment_idx AND TimelineItem.segment_idx <= ?
    ORDER BY TimelineItem.sort_date DESC
    , TimelineItem.taken_date DESC
    , TimelineItem.series_id
    , TimelineItem.group_id DESC
    , TimelineItem.asset_id;
    "#,
    );
    let query = sql_query(qb.finish())
        .bind::<diesel::sql_types::BigInt, _>(segment_min)
        .bind::<diesel::sql_types::BigInt, _>(segment_max);
    let query_start = Instant::now();
    let rows: Vec<RowTimelineSegmentInSection> = query
        .load(conn)
        .wrap_err("error querying timeline segments in section")?;
    let query_elapsed = query_start.elapsed();

    let processing_start = Instant::now();
    let segments: Vec<TimelineSegment> = rows
        .into_iter()
        .group_by(|row| row.segment_idx)
        .into_iter()
        .map(|(segment_idx, segment_rows)| {
            let mut first_row: Option<_> = None;
            let mut items: Vec<AssetsInTimeline> = Vec::default();
            for row in segment_rows {
                if first_row.is_none() {
                    first_row = Some(row.clone());
                }
                // TODO: additional query per row is not great
                let asset: Asset = repository::asset::get_asset(conn, AssetId(row.asset.asset_id))?;
                match (
                    row.series_id,
                    row.series_date,
                    row.series_len,
                    row.is_series_selection,
                ) {
                    (None, None, None, None) => {
                        items.push(AssetsInTimeline::Asset(asset));
                    }
                    (
                        Some(series_id),
                        Some(series_date),
                        Some(series_len),
                        Some(is_series_selection),
                    ) => {
                        match items.last_mut() {
                            Some(AssetsInTimeline::AssetSeries {
                                assets: series_assets,
                                series_id: prev_series_id,
                                series_date: _,
                                selection_indices,
                                total_series_size: _,
                            }) if series_id == prev_series_id.0 => {
                                // still same series, add this asset to it
                                if is_series_selection != 0 {
                                    selection_indices.push(series_assets.len());
                                }
                                series_assets.push(asset);
                            }
                            _ => {
                                // new series
                                items.push(AssetsInTimeline::AssetSeries {
                                    assets: vec![asset],
                                    series_id: AssetSeriesId(series_id),
                                    series_date: datetime_from_db_repr(series_date)?,
                                    selection_indices: if is_series_selection != 0 {
                                        vec![0]
                                    } else {
                                        vec![]
                                    },
                                    total_series_size: series_len
                                        .try_into()
                                        .expect("COUNT(...) is >= 0"),
                                });
                            }
                        }
                    }
                    other => {
                        return Err(eyre!(
                            "illegal result row: series columns must be all null or all non-null: {:?}", other
                        ));
                    }
                }
            }
            let first_row = first_row.expect(
                "set to Some in first loop iteration, group_by does not produce empty lists",
            );

            let segment_type = match first_row.timeline_group_id {
                None => TimelineSegmentType::DateRange {
                    start: match items.first().expect("list can never by empty") {
                        AssetsInTimeline::Asset(asset) => asset.base.taken_date,
                        AssetsInTimeline::AssetSeries {
                            assets: _,
                            series_id: _,
                            series_date,
                            selection_indices: _,
                            total_series_size: _,
                        } => *series_date,
                    },
                    end: match items.last().expect("list can never by empty") {
                        AssetsInTimeline::Asset(asset) => asset.base.taken_date,
                        AssetsInTimeline::AssetSeries {
                            assets: _,
                            series_id: _,
                            series_date,
                            selection_indices: _,
                            total_series_size: _,
                        } => *series_date,
                    },
                },
                Some(timeline_group_id) => {
                    let group = get_timeline_group(conn, TimelineGroupId(timeline_group_id))?;
                    TimelineSegmentType::Group(TimelineGroupType::UserCreated(group))
                }
            };
            Ok(TimelineSegment {
                ty: segment_type,
                sort_date: datetime_from_db_repr(first_row.sort_date)?,
                items,
                id: segment_idx,
            })
        })
        .try_collect()?;
    let processing_elapsed = processing_start.elapsed();
    tracing::debug!(?query_elapsed, ?processing_elapsed);
    debug_assert!(
        segments.iter().all(
            |segment| segment
                .items
                .iter()
                .rev()
                .is_sorted_by_key(|asset| match asset {
                    AssetsInTimeline::Asset(asset) => asset.base.taken_date,
                    // assets withinin series can have any taken_date, but the series_date should be in
                    // sort order
                    AssetsInTimeline::AssetSeries {
                        assets: _,
                        series_id: _,
                        series_date,
                        selection_indices: _,
                        total_series_size: _,
                    } => *series_date,
                })
        ),
        "assets within TimelineSegment are not sorted by taken_date/series_date descending"
    );
    segments.iter().for_each(|segment| {
        segment.items.iter().for_each(|asset| match asset {
            AssetsInTimeline::Asset(_) => {}
            AssetsInTimeline::AssetSeries {
                assets,
                series_id: _,
                series_date: _,
                selection_indices,
                total_series_size: _,
            } => {
                debug_assert!(
                    assets
                        .iter()
                        .rev()
                        .is_sorted_by_key(|asset| asset.base.taken_date),
                    "assets within AssetSeries are not sorted by taken_date descending"
                );
                debug_assert!(
                    selection_indices.iter().all_unique(),
                    "AssetSeries selection_indices has duplicates"
                );
                debug_assert!(
                    selection_indices.iter().copied().all(|i| i < assets.len()),
                    "AssetSeries selection_indices out of range"
                );
            }
        });
    });
    debug_assert!(
        segments
            .iter()
            .map(|segment| !segment.items.is_empty())
            .all(|b| b),
        "TimelineSegment assets must not be empty"
    );
    debug_assert!(
        segments
            .iter()
            .map(|segment| match segment
                .items
                .first()
                .expect("segment assets must not be empty")
            {
                AssetsInTimeline::Asset(asset) => asset,
                AssetsInTimeline::AssetSeries {
                    assets,
                    series_id: _,
                    series_date: _,
                    selection_indices: _,
                    total_series_size: _,
                } => assets.first().expect("can not be empty"),
            }
            .base
            .taken_date
                == segment.sort_date)
            .all(|b| b),
        "TimelineSegment sort_date is not taken_date of first (most recent) asset"
    );
    Ok(segments)
}
