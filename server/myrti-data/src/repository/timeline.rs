use std::time::Instant;

use chrono::{DateTime, Datelike, Utc};
use diesel::{
    RunQueryDsl, Selectable, SelectableHelper,
    connection::SimpleConnection,
    deserialize::{Queryable, QueryableByName},
    prelude::*,
    query_builder::{QueryBuilder, QueryFragment},
    sql_query,
    sqlite::SqliteQueryBuilder,
};
use eyre::{Context, Result, eyre};
use itertools::Itertools;
use tracing::instrument;

use super::{
    db_entity::{DbAssetFile, DbImageFile, DbVideoFile},
    schema,
    util::datetime_from_db_repr,
};
use crate::model::{
    self, Asset, AssetSeriesId, AssetSpe, FileId, Image, InSeries, TimelineGroup, TimelineGroupId,
    TimelineSectionId,
};

use super::{db_entity::DbAsset, timeline_group::get_timeline_group};
use crate::db::DbConn;

#[tracing::instrument(skip(conn), level = "debug")]
pub fn rebuild_timeline_full(conn: &mut DbConn) -> Result<()> {
    conn.immediate_transaction(|conn| {
        conn.batch_execute(include_str!("rebuild_timeline_prolog.sql"))
            .wrap_err("error executing rebuild_timeline_prolog query")?;
        conn.batch_execute(include_str!("rebuild_timeline_full.sql"))
            .wrap_err("error executing rebuild_timeline_full query")?;
        conn.batch_execute(include_str!("rebuild_timeline_common.sql"))
            .wrap_err("error executing rebuild_timeline_common query")?;
        conn.batch_execute(include_str!("rebuild_timeline_months.sql"))
            .wrap_err("error executing rebuild_timeline_months query")?;
        Ok(())
    })
}

#[tracing::instrument(skip(conn), level = "debug")]
pub fn update_timeline_dirty(conn: &mut DbConn) -> Result<()> {
    conn.immediate_transaction(|conn| {
        conn.batch_execute(include_str!("rebuild_timeline_prolog.sql"))
            .wrap_err("error executing rebuild_timeline_prolog query")?;
        conn.batch_execute(include_str!("rebuild_dirty_sections.sql"))
            .wrap_err("error executing rebuild_dirty_sections query")?;
        conn.batch_execute(include_str!("rebuild_timeline_common.sql"))
            .wrap_err("error executing rebuild_timeline_common query")?;
        conn.batch_execute(include_str!("rebuild_timeline_months.sql"))
            .wrap_err("error executing rebuild_timeline_months query")?;
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

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSection {
    pub id: TimelineSectionId,
    pub num_assets: i32,
    /// date of *most recent* asset in section's segments
    pub start_date: DateTime<Utc>,
    /// date of *oldest* asset in section's segments
    pub end_date: DateTime<Utc>,
    pub total_normalized_width: f64,
}

#[derive(Debug, Clone, Selectable, Queryable)]
#[diesel(table_name = super::schema::TimelineSection)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct RowTimelineSection {
    pub section_idx: i64,
    pub section_len: i32,
    pub total_width: f64,
}

#[tracing::instrument(skip(conn), level = "debug")]
pub fn get_sections(conn: &mut DbConn) -> Result<Vec<TimelineSection>> {
    use schema::TimelineSection as Table;
    let rows: Vec<RowTimelineSection> = Table::table
        .group_by(Table::section_idx)
        .select(RowTimelineSection::as_select())
        .load(conn)
        .wrap_err("error querying table TimelineSection")?;
    let sections = rows
        .into_iter()
        .map(|row| {
            Ok(TimelineSection {
                id: TimelineSectionId(row.section_idx),
                start_date: datetime_from_db_repr(0)?,
                end_date: datetime_from_db_repr(10)?,
                num_assets: row.section_len,
                total_normalized_width: row.total_width,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(sections)
}

#[derive(Debug, Copy, Clone)]
pub struct TimelineMonth {
    pub year: i32,
    pub month: i32,
    pub section_idx: i64,
    pub num_assets: i32,
    pub total_normalized_width: f64,
}

#[derive(Debug, Clone, Selectable, Queryable)]
#[diesel(table_name = super::schema::TimelineMonth)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct RowTimelineMonth {
    pub start_of_month: i64,
    pub section_idx: i64,
    pub num_assets: i32,
    pub total_width: f64,
}

#[tracing::instrument(skip(conn), level = "debug")]
pub fn get_months(conn: &mut DbConn) -> Result<Vec<TimelineMonth>> {
    use schema::TimelineMonth as Table;
    let rows: Vec<RowTimelineMonth> = Table::table
        .order_by((Table::section_idx, Table::start_of_month.desc()))
        .select(RowTimelineMonth::as_select())
        .load(conn)
        .wrap_err("error querying table TimelineMonth")?;
    rows.into_iter()
        .map(|row| {
            let date =
                chrono::DateTime::from_timestamp(row.start_of_month, 0).ok_or_else(|| {
                    eyre!(
                        "could not convert timestamp {} to Utc DateTime",
                        row.start_of_month
                    )
                })?;

            Ok::<_, eyre::Error>(TimelineMonth {
                year: date.year(),
                month: date.month().try_into()?,
                section_idx: row.section_idx,
                num_assets: row.num_assets,
                total_normalized_width: row.total_width,
            })
        })
        .try_collect()
        .wrap_err("error getting timeline month summaries")
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
    pub id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetInTimelineExtra {
    Image { representations: String },
    Video {},
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetsInTimeline {
    Asset(Asset, AssetInTimelineExtra),
    AssetSeries {
        assets: Vec<(Asset, AssetInTimelineExtra)>,
        series_id: AssetSeriesId,
        series_date: DateTime<Utc>,
        selection_indices: Vec<usize>,
    },
}

#[derive(Debug, Clone, Queryable, QueryableByName, Selectable)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[diesel(table_name = super::schema::TimelineItem)]
#[allow(dead_code)]
struct DbTimelineItem {
    pub asset_id: i64,
    pub series_id: Option<i64>,
    pub series_date: Option<i64>,
    pub group_id: Option<i64>,
    pub group_date: Option<i64>,
    pub sort_date: i64,
    pub section_idx: i32,
    pub segment_id: i32,
    pub segment_split_idx: Option<i32>,
}

#[derive(Debug, Clone, Queryable, QueryableByName, Selectable)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[allow(dead_code)]
struct RowTimelineSegmentInSection {
    #[diesel(embed)]
    pub timeline_item: DbTimelineItem,
    #[diesel(embed)]
    pub asset: DbAsset,
    #[diesel(embed)]
    pub asset_file: DbAssetFile,
    #[diesel(embed)]
    pub video_file: Option<DbVideoFile>,
    #[diesel(embed)]
    pub image_file: Option<DbImageFile>,
    // #[diesel(sql_type = diesel::sql_types::Nullable::<diesel::sql_types::Text>)]
    // #[diesel(column_name = image_representations)]
    // pub image_representations: Option<String>,
}

#[derive(Debug, Clone, Queryable, QueryableByName)]
struct ImageReprColumn {
    #[diesel(sql_type = diesel::sql_types::Nullable::<diesel::sql_types::Text>)]
    pub image_representations: Option<String>,
}

#[instrument(err(Debug), skip(conn), level = "debug")]
pub fn get_segments_in_section(
    conn: &mut DbConn,
    section_idx: TimelineSectionId,
) -> Result<Vec<TimelineSegment>> {
    let mut qb = SqliteQueryBuilder::new();
    qb.push_sql(
        r#"
    SELECT
    "#,
    );
    RowTimelineSegmentInSection::as_select().to_sql(&mut qb, &diesel::sqlite::Sqlite)?;
    qb.push_sql(
        r#"
        , image_representations
    FROM
    TimelineItem INNER JOIN Asset ON Asset.asset_id = TimelineItem.asset_id
    INNER JOIN AssetFile ON AssetFile.file_id = Asset.rep_file_id
    LEFT JOIN ImageFile ON ImageFile.file_id = AssetFile.file_id
    LEFT JOIN VideoFile ON VideoFile.file_id = AssetFile.file_id
    LEFT JOIN (
        SELECT ImageRepresentation.file_id
        , json_group_array(
            json_object(
                'id', CAST(ImageRepresentation.image_repr_id AS TEXT)
                , 'format', ImageRepresentation.format_name
                , 'width', ImageRepresentation.width
                , 'height', ImageRepresentation.height
                , 'size', ImageRepresentation.file_size
            )
        ) AS image_representations
        FROM ImageRepresentation
        GROUP BY ImageRepresentation.file_id
    ) json_sub ON json_sub.file_id = AssetFile.file_id
    WHERE
    TimelineItem.section_idx = ?
    ORDER BY TimelineItem.sort_date DESC
    , TimelineItem.taken_date DESC
    , TimelineItem.segment_id
    , TimelineItem.series_id
    , TimelineItem.group_id DESC
    , TimelineItem.asset_id;
    "#,
    );
    let query = sql_query(qb.finish()).bind::<diesel::sql_types::BigInt, _>(section_idx.0);
    let query_start = Instant::now();
    let rows: Vec<(RowTimelineSegmentInSection, ImageReprColumn)> = query
        .load(conn)
        .wrap_err("error querying timeline items in section")?;
    let query_elapsed = query_start.elapsed();

    let processing_start = Instant::now();
    let segments: Vec<TimelineSegment> = rows
        .into_iter()
        .group_by(|(row, _)| row.timeline_item.segment_id)
        .into_iter()
        .map(|(segment_id, segment_rows)| {
            let mut first_row: Option<_> = None;
            let mut items: Vec<AssetsInTimeline> = Vec::default();
            for (row, image_repr_json) in segment_rows {
                if first_row.is_none() {
                    first_row = Some(row.clone());
                }
                let asset_base: model::AssetBase = row.asset.try_into()?;
                let (sp, extra) = match (row.image_file, row.video_file, image_repr_json.image_representations) {
                    (Some(image), None, image_repr_json) => {
                        let sp =AssetSpe::Image(Image {
                            file_id: FileId(row.asset_file.file_id),
                            image_format_name: image.image_format_name,
                        });
                        let extra = AssetInTimelineExtra::Image { representations: image_repr_json.unwrap_or("[]".to_owned()) };
                        (sp, extra)
                    }
                    (None, Some(video), None) => {
                        (AssetSpe::Video(video.try_into()?), AssetInTimelineExtra::Video {  })
                    }
                    _ => panic!("AssetFile row has no matching ImageFile or VideoFile row")
                };

                let asset = model::Asset {
                    base: asset_base,
                    rep_file: row.asset_file.try_into()?,
                    sp,
                };
                match (
                    asset.base.in_series,
                    row.timeline_item.series_date,
                ) {
                    (None, None) => {
                        items.push(AssetsInTimeline::Asset(asset, extra));
                    }
                    (
                        Some(InSeries {series_id, is_selection}),
                        Some(series_date),
                    ) => {
                        match items.last_mut() {
                            Some(AssetsInTimeline::AssetSeries {
                                assets: series_assets,
                                series_id: prev_series_id,
                                series_date: _,
                                selection_indices,
                            }) if series_id == *prev_series_id => {
                                // still same series, add this asset to it
                                if is_selection {
                                    selection_indices.push(series_assets.len());
                                }
                                series_assets.push((asset, extra));
                            }
                            _ => {
                                // new series
                                items.push(AssetsInTimeline::AssetSeries {
                                    assets: vec![(asset, extra)],
                                    series_id ,
                                    series_date: datetime_from_db_repr(series_date)?,
                                    selection_indices: if is_selection  {
                                        vec![0]
                                    } else {
                                        vec![]
                                    },
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

            let segment_type = match first_row.timeline_item.group_id {
                None => TimelineSegmentType::DateRange {
                    start: match items.first().expect("list can never by empty") {
                        AssetsInTimeline::Asset(asset, _) => asset.base.taken_date,
                        AssetsInTimeline::AssetSeries {
                            assets: _,
                            series_id: _,
                            series_date,
                            selection_indices: _,
                        } => *series_date,
                    },
                    end: match items.last().expect("list can never by empty") {
                        AssetsInTimeline::Asset(asset, _) => asset.base.taken_date,
                        AssetsInTimeline::AssetSeries {
                            assets: _,
                            series_id: _,
                            series_date,
                            selection_indices: _,
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
                sort_date: datetime_from_db_repr(first_row.timeline_item.sort_date)?,
                items,
                id: segment_id,
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
                    AssetsInTimeline::Asset(asset, _) => asset.base.taken_date,
                    // assets withinin series can have any taken_date, but the series_date should be in
                    // sort order
                    AssetsInTimeline::AssetSeries {
                        assets: _,
                        series_id: _,
                        series_date,
                        selection_indices: _,
                    } => *series_date,
                })
        ),
        "assets within TimelineSegment are not sorted by taken_date/series_date descending"
    );
    segments.iter().for_each(|segment| {
        segment.items.iter().for_each(|asset| match asset {
            AssetsInTimeline::Asset(..) => {}
            AssetsInTimeline::AssetSeries {
                assets,
                series_id: _,
                series_date: _,
                selection_indices,
            } => {
                debug_assert!(
                    assets
                        .iter()
                        .rev()
                        .is_sorted_by_key(|(asset, _)| asset.base.taken_date),
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
        segments.iter().all(|segment| !segment.items.is_empty()),
        "TimelineSegment assets must not be empty"
    );
    for segment in &segments {
        let most_recent_date = match segment
            .items
            .first()
            .expect("segment assets must not be empty")
        {
            AssetsInTimeline::Asset(asset, _) => asset,
            AssetsInTimeline::AssetSeries {
                assets,
                series_id: _,
                series_date: _,
                selection_indices: _,
            } => &assets.first().expect("can not be empty").0,
        }
        .base
        .taken_date;
        debug_assert_eq!(
            most_recent_date, segment.sort_date,
            "TimelineSegment sort_date is not taken_date of first (most recent) asset:\n{:?}",
            segment
        );
    }
    Ok(segments)
}
