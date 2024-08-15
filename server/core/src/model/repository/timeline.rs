use chrono::{DateTime, Utc};
use const_format::formatcp;
use diesel::{
    deserialize::QueryableByName,
    query_builder::{QueryBuilder, QueryFragment},
    sql_query,
    sqlite::SqliteQueryBuilder,
    RunQueryDsl, SelectableHelper,
};
use eyre::{eyre, Context, Result};
use is_sorted::IsSorted;
use itertools::Itertools;
use tracing::instrument;

use crate::model::{
    util::datetime_from_db_repr, Asset, AssetId, AssetSeriesId, TimelineGroup, TimelineGroupId,
};

use super::{db::DbConn, db_entity::DbAsset, timeline_group::get_timeline_group};

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

#[derive(Debug, Clone, QueryableByName)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct RowAssetGroupId {
    #[diesel(embed)]
    pub asset: DbAsset,
    #[diesel(column_name = group_id)]
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    pub group_id: Option<i64>,
    #[diesel(column_name = sort_group_date)]
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub sort_group_date: i64,
}

// TODO when the time comes: figure out how to handle dates of different timezones.
// Right now assets are grouped by Asset::taken_date_local().date_naive() and sorted by timestamp,
// which may or may not by what we want.
#[tracing::instrument(skip(conn))]
pub fn get_timeline_chunk(
    conn: &mut DbConn,
    last_id: Option<AssetId>,
    max_count: i64,
) -> Result<Vec<TimelineElement>> {
    // timezone to calculate local dates in
    let timezone = &chrono::Local;
    let mut qb = SqliteQueryBuilder::new();
    qb.push_sql(r#"
    WITH last_asset AS
    (
        SELECT Asset.*,
        CASE WHEN TimelineGroup.timeline_group_id IS NOT NULL THEN TimelineGroup.timeline_group_id ELSE 0 END AS album_id,
        CASE WHEN TimelineGroup.timeline_group_id IS NOT NULL THEN TimelineGroup.display_date ELSE Asset.taken_date END AS sort_group_date
        FROM Asset
        LEFT JOIN TimelineGroupItem ON TimelineGroupItem.asset_id = Asset.asset_id
        LEFT JOIN TimelineGroup ON TimelineGroupItem.group_id = TimelineGroup.timeline_group_id
        WHERE Asset.asset_id = $1
    )
    SELECT
    "#);
    DbAsset::as_select().to_sql(&mut qb, &diesel::sqlite::Sqlite)?;
    qb.push_sql(r#"
    ,
    CASE WHEN TimelineGroup.timeline_group_id IS NOT NULL THEN TimelineGroup.timeline_group_id ELSE NULL END AS group_id,
    CASE WHEN TimelineGroup.timeline_group_id IS NOT NULL THEN TimelineGroup.display_date ELSE Asset.taken_date END AS sort_group_date
    FROM Asset
    LEFT JOIN TimelineGroupItem ON TimelineGroupItem.asset_id = Asset.asset_id
    LEFT JOIN TimelineGroup ON TimelineGroupItem.group_id = TimelineGroup.timeline_group_id
    WHERE
    (
        ($1 IS NULL)
        OR
        (sort_group_date, group_id, Asset.taken_date, Asset.asset_id) < (SELECT sort_group_date, album_id, taken_date, asset_id FROM last_asset)
        OR
        (
        group_id IS NULL AND (SELECT album_id FROM last_asset) IS NULL
        AND
        (sort_group_date, Asset.taken_date, Asset.asset_id) < (SELECT sort_group_date, taken_date, asset_id FROM last_asset)
        )
    )
    ORDER BY sort_group_date DESC, group_id DESC, Asset.taken_date DESC, Asset.asset_id DESC
    LIMIT $2;
    "#);
    use diesel::sql_types::{BigInt, Nullable};
    let assets_groupid: Vec<RowAssetGroupId> = sql_query(qb.finish())
        .bind::<Nullable<BigInt>, _>(last_id.map(|id| id.0))
        .bind::<BigInt, _>(max_count)
        .load(conn)?;
    let mut timeline_els: Vec<TimelineElement> = Vec::default();
    for row in assets_groupid {
        let asset: Asset = row.asset.try_into()?;
        let group_id = row.group_id.map(TimelineGroupId);
        // let sort_group_date = datetime_from_db_repr(row.sort_group_date)?;
        let mut last_el = timeline_els.last_mut();
        match &mut last_el {
            None => {
                // create new TimelineElement
                let new_el = if let Some(group_id) = group_id {
                    let group = get_timeline_group(conn, group_id)?;
                    TimelineElement::Group {
                        group,
                        assets: vec![asset],
                    }
                } else {
                    TimelineElement::DayGrouped(vec![asset])
                };
                timeline_els.push(new_el);
            }
            Some(ref mut last_el) => match (last_el, group_id) {
                // Matching cases: add this asset to last TimelineElement
                (TimelineElement::DayGrouped(ref mut assets), None)
                    if assets
                        .last()
                        .map(|a| {
                            a.base.taken_date.with_timezone(timezone).date_naive()
                                == asset.base.taken_date.with_timezone(timezone).date_naive()
                        })
                        // .unwrap_or(true)
                        .expect("There should never be an empty DayGrouped") =>
                {
                    assets.push(asset);
                }
                (TimelineElement::Group { group, assets }, Some(group_id))
                    if group.id == group_id =>
                {
                    assets.push(asset);
                }
                // Need to create new TimelineElement for these cases
                (_, Some(group_id)) => {
                    let group = get_timeline_group(conn, group_id)?;
                    timeline_els.push(TimelineElement::Group {
                        group,
                        assets: vec![asset],
                    });
                }
                // last DayGroup element does not match this date
                (_, None) => timeline_els.push(TimelineElement::DayGrouped(vec![asset])),
            },
        };
    }
    Ok(timeline_els)
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
    pub min_segment: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub max_segment: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub asset_count: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub oldest_asset_taken_date: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub newest_asset_taken_date: i64,
}

#[tracing::instrument(skip(conn))]
pub fn get_sections(conn: &mut DbConn) -> Result<Vec<TimelineSection>> {
    const SQL_SEGMENT_IDX: &str = include_str!("timeline_segment_idx.sql");
    const QUERY: &str = formatcp!(
        r#"
    WITH tl_segment_idx AS ({SQL_SEGMENT_IDX}),
    segment_size AS
    (
        SELECT *, COUNT(asset_id) AS segment_size FROM tl_segment_idx GROUP BY segment_idx
    ),
    cumsum_segment_size AS (
        SELECT *, SUM(segment_size) OVER (PARTITION BY 1 ORDER BY segment_idx ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS cum_segment_size FROM segment_size
    ),
    section_segments AS (
        SELECT 
        cum_segment_size / 100 as section_idx,
        MIN(segment_idx) as min_segment,
        MAX(segment_idx) as max_segment,
        SUM(segment_size) as asset_count
        FROM cumsum_segment_size
        GROUP BY section_idx
    )
    SELECT
    section_idx,
    min_segment,
    max_segment,
    asset_count,
    (
        SELECT MAX(tl_segment_idx.taken_date)
        FROM tl_segment_idx
        WHERE section_segments.min_segment = tl_segment_idx.segment_idx
        GROUP BY tl_segment_idx.segment_idx
    ) AS newest_asset_taken_date,
    (
        SELECT MIN(tl_segment_idx.taken_date)
        FROM tl_segment_idx
        WHERE section_segments.max_segment = tl_segment_idx.segment_idx
        GROUP BY tl_segment_idx.segment_idx
    ) AS oldest_asset_taken_date
    FROM section_segments;
    "#
    );
    let rows: Vec<RowTimelineSection> = sql_query(QUERY).load(conn)?;
    let sections = rows
        .into_iter()
        .map(|row| {
            Ok(TimelineSection {
                id: TimelineSectionId {
                    segment_min: row.min_segment,
                    segment_max: row.max_segment,
                },
                start_date: datetime_from_db_repr(row.newest_asset_taken_date)?,
                end_date: datetime_from_db_repr(row.oldest_asset_taken_date)?,
                num_assets: row.asset_count,
            })
        })
        .collect::<Result<Vec<_>>>()?;
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
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub sort_date_day: Option<String>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub segment_idx: i64,
}

#[instrument(err, skip(conn))]
pub fn get_segments_in_section(
    conn: &mut DbConn,
    segment_min: i64,
    segment_max: i64,
) -> Result<Vec<TimelineSegment>> {
    const SQL_SEGMENT_IDX: &str = include_str!("timeline_segment_idx.sql");
    let mut qb = SqliteQueryBuilder::new();
    qb.push_sql(formatcp!(r#"WITH tl_segment_idx AS ({SQL_SEGMENT_IDX})"#));
    qb.push_sql(
        r#"
    SELECT
    "#,
    );
    DbAsset::as_select().to_sql(&mut qb, &diesel::sqlite::Sqlite)?;
    qb.push_sql(
        r#"
    ,
    tl_segment_idx.group_id as timeline_group_id,
    tl_segment_idx.series_id as series_id,
    tl_segment_idx.series_date as series_date,
    tl_segment_idx.series_len as series_len,
    Asset.is_series_selection as is_series_selection,
    tl_segment_idx.sort_date as sort_date,
    tl_segment_idx.sort_date_day as sort_date_day,
    tl_segment_idx.segment_idx as segment_idx
    FROM
    tl_segment_idx INNER JOIN Asset ON Asset.asset_id = tl_segment_idx.asset_id
    WHERE
    ? <= segment_idx AND segment_idx <= ?
    ORDER BY tl_segment_idx.sort_date DESC, tl_segment_idx.series_date DESC, tl_segment_idx.taken_date DESC,
    tl_segment_idx.series_id, tl_segment_idx.group_id DESC, tl_segment_idx.asset_id;
    "#,
    );
    let query = sql_query(qb.finish())
        .bind::<diesel::sql_types::BigInt, _>(segment_min)
        .bind::<diesel::sql_types::BigInt, _>(segment_max);
    let rows: Vec<RowTimelineSegmentInSection> = query
        .load(conn)
        .wrap_err("error querying timeline segments in section")?;
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
                let asset: Asset = row.asset.try_into()?;
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
    debug_assert!(
        segments
            .iter()
            .map(|segment| segment
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
                }))
            .all(|b| b),
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
