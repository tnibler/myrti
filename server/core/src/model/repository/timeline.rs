use chrono::{DateTime, Utc};
use eyre::{Context, Result};

use crate::model::{
    repository::db_entity::{DbAsset, DbAssetType, DbTimestampInfo},
    Asset, AssetId, AssetRootDirId, TimelineGroup, TimelineGroupId,
};

use super::{pool::DbPool, timeline_group::get_timeline_group};

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
            TimelineElement::DayGrouped(assets) => &assets,
            TimelineElement::Group { group: _, assets } => &assets,
        }
    }
}

macro_rules! dbasset_from_row {
    (&$row:ident) => {
        DbAsset {
            id: AssetId($row.id),
            ty: $row.ty,
            root_dir_id: AssetRootDirId($row.root_dir_id),
            file_type: $row.file_type,
            file_path: $row.file_path,
            is_hidden: $row.is_hidden,
            hash: $row.hash,
            added_at: $row.added_at,
            taken_date: $row.taken_date,
            timezone_offset: $row.timezone_offset,
            timezone_info: $row.timezone_info,
            width: $row.width,
            height: $row.height,
            rotation_correction: $row.rotation_correction,
            gps_latitude: $row.gps_latitude,
            gps_longitude: $row.gps_longitude,
            thumb_small_square_avif: $row.thumb_small_square_avif,
            thumb_small_square_webp: $row.thumb_small_square_webp,
            thumb_large_orig_avif: $row.thumb_large_orig_avif,
            thumb_large_orig_webp: $row.thumb_large_orig_webp,
            thumb_small_square_width: $row.thumb_small_square_width,
            thumb_small_square_height: $row.thumb_small_square_height,
            thumb_large_orig_width: $row.thumb_large_orig_width,
            thumb_large_orig_height: $row.thumb_large_orig_height,
            image_format_name: $row.image_format_name,
            video_codec_name: $row.video_codec_name,
            video_bitrate: $row.video_bitrate,
            audio_codec_name: $row.audio_codec_name,
            has_dash: $row.has_dash,
        }
    };
}

// TODO when the time comes: figure out how to handle dates of different timezones.
// Right now assets are grouped by Asset::taken_date_local().date_naive() and sorted by timestamp,
// which may or may not by what we want.
#[tracing::instrument(skip(pool), level = "debug")]
pub async fn get_timeline_chunk(
    pool: &DbPool,
    last_id: Option<AssetId>,
    max_count: i64,
) -> Result<Vec<TimelineElement>> {
    // timezone to calculate local dates in
    let timezone = &chrono::Local;
    let assets_groupid = sqlx::query!(
        r#"
WITH last_asset AS
(
    SELECT Asset.*, 
    CASE WHEN TimelineGroup.id IS NOT NULL THEN TimelineGroup.id ELSE 0 END AS album_id,
    CASE WHEN TimelineGroup.id IS NOT NULL THEN TimelineGroup.display_date ELSE Asset.taken_date END AS sort_group_date
    FROM Asset
    LEFT JOIN TimelineGroupEntry ON TimelineGroupEntry.asset_id = Asset.id
    LEFT JOIN TimelineGroup ON TimelineGroupEntry.group_id = TimelineGroup.id
    WHERE Asset.id = $1
)
SELECT
Asset.id,
Asset.ty as "ty: DbAssetType",
Asset.root_dir_id,
Asset.file_path,
Asset.file_type,
Asset.hash,
Asset.is_hidden,
Asset.added_at,
Asset.taken_date,
Asset.timezone_offset,
Asset.timezone_info as "timezone_info: DbTimestampInfo",
Asset.width,
Asset.height,
Asset.rotation_correction as "rotation_correction: i64",
Asset.gps_latitude as "gps_latitude: i64",
Asset.gps_longitude as "gps_longitude: i64",
Asset.thumb_small_square_avif ,
Asset.thumb_small_square_webp, 
Asset.thumb_large_orig_avif ,
Asset.thumb_large_orig_webp ,
Asset.thumb_small_square_width,
Asset.thumb_small_square_height,
Asset.thumb_large_orig_width,
Asset.thumb_large_orig_height,
Asset.image_format_name,
Asset.video_codec_name,
Asset.video_bitrate,
Asset.audio_codec_name,
Asset.has_dash,
CASE WHEN TimelineGroup.id IS NOT NULL THEN TimelineGroup.id ELSE 0 END AS group_id,
CASE WHEN TimelineGroup.id IS NOT NULL THEN TimelineGroup.display_date ELSE Asset.taken_date END AS "sort_group_date: i64"
FROM Asset
LEFT JOIN TimelineGroupEntry ON TimelineGroupEntry.asset_id = Asset.id
LEFT JOIN TimelineGroup ON TimelineGroupEntry.group_id = TimelineGroup.id
WHERE
(
    ($1 IS NULL)
    OR 
    ("sort_group_date: i64", group_id, Asset.taken_date, Asset.id) < (SELECT sort_group_date, album_id, taken_date, id FROM last_asset)
    OR
    (
    group_id IS NULL AND (SELECT album_id FROM last_asset) IS NULL
    AND
    ("sort_group_date: i64", Asset.taken_date, Asset.id) < (SELECT sort_group_date, taken_date, id FROM last_asset)
    )
)
ORDER BY "sort_group_date: i64" DESC, group_id DESC, Asset.taken_date DESC, Asset.id DESC
LIMIT $2;
    "#,
        last_id,
        max_count
    )
        .fetch_all(pool)
        .await
        .wrap_err("timeline query failed")?
        .into_iter()
        .map(|row| {
            // println!("{:?} {:?} {:?} {:?}", row.id, row.sort_group_date, row.album_id, row.taken_date);
            let asset = dbasset_from_row!(&row);
            let group_id = match row.group_id {
                0 => None,
                id => Some(TimelineGroupId(id as i64))
            };
            (asset, group_id)
        });
    let mut timeline_els: Vec<TimelineElement> = Vec::default();
    for (db_asset, group_id) in assets_groupid {
        let asset: Asset = db_asset.try_into()?;
        let mut last_el = timeline_els.last_mut();
        match &mut last_el {
            None => {
                // create new TimelineElement
                let new_el = if let Some(group_id) = group_id {
                    let group = get_timeline_group(pool, group_id).await?;
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
                    let group = get_timeline_group(pool, group_id).await?;
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

#[tracing::instrument(skip(pool), level = "debug")]
pub async fn get_sections(pool: &DbPool) -> Result<Vec<TimelineSection>> {
    // Right now a section always contains entire segments,
    // even if
    let sections: Vec<TimelineSection> = sqlx::query!(
        r#"
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
section_idx as "section_idx: i64",
MIN(segment_idx) AS "min_segment: i64",
MAX(segment_idx) AS "max_segment: i64",
SUM(asset_count) as "asset_count: i64"
FROM segment_sections
GROUP BY section_idx;
    "#
    ).fetch_all(pool)
    .await
    .wrap_err("error in timeline section query")?
    .into_iter()
    .map(|row| TimelineSection {
        id: TimelineSectionId { segment_min: row.min_segment, segment_max: row.max_segment },
        num_assets: row.asset_count
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

pub async fn get_segments_in_section(
    pool: &DbPool,
    segment_min: i64,
    segment_max: i64,
) -> Result<Vec<TimelineSegment>> {
    let rows = sqlx::query!(
    // copy-pasted from above, figure out how to keep them in sync later.
    // probably creating non-temporary views or actual tables
        r#"
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
Asset.id,
Asset.ty as "ty: DbAssetType",
Asset.root_dir_id,
Asset.file_path,
Asset.file_type,
Asset.hash,
Asset.is_hidden,
Asset.added_at,
Asset.taken_date,
Asset.timezone_offset,
Asset.timezone_info as "timezone_info: DbTimestampInfo",
Asset.width,
Asset.height,
Asset.rotation_correction as "rotation_correction: i64",
Asset.gps_latitude as "gps_latitude: i64",
Asset.gps_longitude as "gps_longitude: i64",
Asset.thumb_small_square_avif ,
Asset.thumb_small_square_webp, 
Asset.thumb_large_orig_avif ,
Asset.thumb_large_orig_webp ,
Asset.thumb_small_square_width,
Asset.thumb_small_square_height,
Asset.thumb_large_orig_width,
Asset.thumb_large_orig_height,
Asset.image_format_name,
Asset.video_codec_name,
Asset.video_bitrate,
Asset.audio_codec_name,
Asset.has_dash,
segment.timeline_group_id as "timeline_group_id?",
segment.date_day as "date_day: Option<String>",
segment.segment_idx
FROM 
segment INNER JOIN Asset ON Asset.id=asset_id
WHERE 
? <= segment_idx AND segment_idx <= ?;
    "#,
        segment_min,
        segment_max
    ).fetch_all(pool)
    .await
    .wrap_err("error in timeline section query")?;
    let mut segments: Vec<TimelineSegment> = Vec::new();
    for row in rows {
        let asset: Asset = dbasset_from_row!(&row).try_into()?;
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
                        let group = get_timeline_group(pool, TimelineGroupId(group_id)).await?;
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
                        let group = get_timeline_group(pool, TimelineGroupId(group_id)).await?;
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
