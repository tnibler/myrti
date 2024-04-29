use chrono::{DateTime, Utc};
use diesel::{
    deserialize::QueryableByName,
    query_builder::{QueryBuilder, QueryFragment},
    sql_query,
    sqlite::SqliteQueryBuilder,
    RunQueryDsl, SelectableHelper,
};
use eyre::{Context, Result};
use tracing::instrument;

use crate::model::{Asset, AssetId, TimelineGroup, TimelineGroupId};

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
}

#[tracing::instrument(skip(conn))]
pub fn get_sections(conn: &mut DbConn) -> Result<Vec<TimelineSection>> {
    // Right now a section always contains entire segments,
    // even if
    let rows: Vec<RowTimelineSection> = sql_query(r#"
    WITH segment_cumul_size AS (
      SELECT *, SUM(1) OVER (ORDER BY sort_date DESC, timeline_group_id DESC, asset_id DESC) AS cumul_segment_size from `TimelineSegment`
    ),
    segment AS (
      SELECT
      asset_id,
      sort_date,
      timeline_group_id,
      date_day,
      DENSE_RANK() OVER (ORDER BY segment_idx, cumul_segment_size / 30) AS segment_idx
      FROM segment_cumul_size
    ),
    final_segment_size AS (
      SELECT
      segment_idx,
      COUNT(asset_id) AS asset_count
      FROM segment GROUP BY segment_idx
    ),
    segment_sections AS (
    SELECT
    segment_idx,
    asset_count,
    SUM(asset_count) OVER (PARTITION BY 1 ORDER BY segment_idx ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS c,
    SUM(asset_count) OVER (PARTITION BY 1 ORDER BY segment_idx ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) / 100 AS section_idx
    FROM final_segment_size
    )
    SELECT
    section_idx,
    MIN(segment_idx) AS min_segment,
    MAX(segment_idx) AS max_segment,
    SUM(asset_count) as asset_count
    FROM segment_sections
    GROUP BY section_idx;
    "#).load(conn)?;
    let sections = rows
        .into_iter()
        .map(|row| TimelineSection {
            id: TimelineSectionId {
                segment_min: row.min_segment,
                segment_max: row.max_segment,
            },
            num_assets: row.asset_count,
        })
        .collect();
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
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineSegment {
    pub ty: TimelineSegmentType,
    pub assets: Vec<Asset>,
    pub id: i64,
}

#[derive(Debug, Clone, QueryableByName)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct RowTimelineSegmentInSection {
    #[diesel(embed)]
    pub asset: DbAsset,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    pub timeline_group_id: Option<i64>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub date_day: Option<String>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub segment_idx: i64,
}

#[instrument(skip(conn))]
pub fn get_segments_in_section(
    conn: &mut DbConn,
    segment_min: i64,
    segment_max: i64,
) -> Result<Vec<TimelineSegment>> {
    let mut qb = SqliteQueryBuilder::new();
    qb.push_sql(r#"
    WITH segment_cumul_size AS (
      SELECT *, SUM(1) OVER (ORDER BY sort_date DESC, timeline_group_id DESC, asset_id DESC) AS cumul_segment_size from `TimelineSegment`
    ),
    segment AS (
      SELECT
      asset_id,
      sort_date,
      timeline_group_id,
      date_day,
      DENSE_RANK() OVER (ORDER BY segment_idx, cumul_segment_size / 30) AS segment_idx
      FROM segment_cumul_size
    )
    SELECT
    "#);
    DbAsset::as_select().to_sql(&mut qb, &diesel::sqlite::Sqlite)?;
    qb.push_sql(
        r#"
    ,
    segment.timeline_group_id as timeline_group_id,
    segment.date_day as date_day,
    segment.segment_idx as segment_idx
    FROM
    segment INNER JOIN Asset ON Asset.asset_id=segment.asset_id
    WHERE
    ? <= segment_idx AND segment_idx <= ?;
    "#,
    );
    let query = sql_query(qb.finish())
        .bind::<diesel::sql_types::BigInt, _>(segment_min)
        .bind::<diesel::sql_types::BigInt, _>(segment_max);
    let rows: Vec<RowTimelineSegmentInSection> = query
        .load(conn)
        .wrap_err("error querying timeline segments in section")?;
    let mut segments: Vec<TimelineSegment> = Vec::new();
    for row in rows {
        let asset: Asset = row.asset.try_into()?;
        match segments.last_mut() {
            None => {
                assert!(row.segment_idx == segment_min);
                let ty = match row.timeline_group_id {
                    None => {
                        assert!(row.date_day.is_some());
                        TimelineSegmentType::DateRange {
                            start: asset.base.taken_date,
                            end: asset.base.taken_date,
                        }
                    }
                    Some(group_id) => {
                        assert!(row.date_day.is_none());
                        let group = get_timeline_group(conn, TimelineGroupId(group_id))?;
                        TimelineSegmentType::Group(TimelineGroupType::UserCreated(group))
                    }
                };
                let segment = TimelineSegment {
                    ty,
                    assets: vec![asset],
                    id: row.segment_idx,
                };
                segments.push(segment);
            }
            Some(ref mut last_segment) if last_segment.id == row.segment_idx => {
                match &mut last_segment.ty {
                    TimelineSegmentType::Group(TimelineGroupType::UserCreated(group)) => {
                        assert!(
                            row.timeline_group_id
                                .expect("column timeline_group_id must not be null")
                                == group.id.0
                        );
                        // nothing to do
                    }
                    TimelineSegmentType::DateRange {
                        start: _,
                        ref mut end,
                    } => {
                        assert!(
                            asset.base.taken_date <= *end,
                            "next Asset in segment must have taken_date before the one after it"
                        );
                        *end = asset.base.taken_date;
                    }
                };
                last_segment.assets.push(asset);
            }
            Some(_) => {
                let ty = match row.timeline_group_id {
                    None => {
                        assert!(row.date_day.is_some());
                        TimelineSegmentType::DateRange {
                            start: asset.base.taken_date,
                            end: asset.base.taken_date,
                        }
                    }
                    Some(group_id) => {
                        assert!(row.date_day.is_none());
                        let group = get_timeline_group(conn, TimelineGroupId(group_id))?;
                        TimelineSegmentType::Group(TimelineGroupType::UserCreated(group))
                    }
                };
                let segment = TimelineSegment {
                    ty,
                    assets: vec![asset],
                    id: row.segment_idx,
                };
                segments.push(segment);
            }
        }
    }
    Ok(segments)
}
