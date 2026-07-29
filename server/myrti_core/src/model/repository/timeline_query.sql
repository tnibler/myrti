DELETE FROM TimelineItem;
DELETE FROM TimelineSection;
WITH
SeriesDate AS (
	SELECT 
		AssetSeries.series_id
		, MAX(Asset.taken_date) AS series_date -- most recent date becomes this series' sort date
		-- , COUNT(Asset.asset_id) AS series_len
	FROM Asset INNER JOIN AssetSeries
	ON Asset.series_id = AssetSeries.series_id
	GROUP BY AssetSeries.series_id
)
INSERT INTO TimelineItem(
	asset_id
	, taken_date
	, series_id
	, series_date
	, group_id
	, group_date
	, section_idx
	, segment_idx
)
SELECT Asset.asset_id
, Asset.taken_date
, SeriesDate.series_id
, SeriesDate.series_date
, group_id
, group_date
, -1
, -1
FROM Asset
LEFT JOIN (
	SELECT TimelineGroup.timeline_group_id AS group_id
	, TimelineGroup.display_date AS group_date
	, TimelineGroupItem.asset_id
	FROM TimelineGroup INNER JOIN TimelineGroupItem 
	ON TimelineGroup.timeline_group_id = TimelineGroupItem.group_id
) tgi
ON Asset.asset_id = tgi.asset_id
LEFT JOIN SeriesDate
ON Asset.series_id = SeriesDate.series_id
WHERE Asset.is_hidden = 0;

WITH NewSeriesStart AS (
	SELECT asset_id
	, taken_date
	, group_id
	, series_id
	, series_id IS NOT NULL AND lag(series_id) OVER w IS NOT series_id AS is_series_start
	, segment_date
	FROM TimelineItem
	WINDOW w AS (
		ORDER BY segment_date DESC, taken_date DESC, asset_id DESC
		ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
	)
)
, RawSegmentRank AS (
	SELECT *
	, DENSE_RANK() OVER (
		ORDER BY segment_date DESC
		, IFNULL(group_id, 0) DESC
	)
	AS raw_segment_idx
	, SUM(is_series_start) OVER w + SUM (CASE WHEN series_id IS NULL THEN 1 ELSE 0 END) OVER w AS cumul_segment_len
	FROM NewSeriesStart
	WINDOW w AS (
		PARTITION BY segment_date , IFNULL(group_id, 0) -- must be same as dense_rank() above
		ORDER BY segment_date DESC, taken_date DESC, asset_id DESC
		ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
	)
)
, SegmentRank AS (
	SELECT *
	, cumul_segment_len / 100 AS segment_split_idx
	, DENSE_RANK() OVER (
		ORDER BY segment_date DESC
		, IFNULL(group_id, 0) DESC
		, cumul_segment_len / 100 ASC
	)
	AS segment_idx
	FROM RawSegmentRank
)
, SegmentWithSplit AS (
	SELECT asset_id
	, segment_idx
	, CASE WHEN (MAX(segment_split_idx) OVER (PARTITION BY raw_segment_idx)) <> 0
		THEN segment_split_idx
		ELSE NULL
	END AS segment_split_idx
	FROM SegmentRank
)
UPDATE TimelineItem
SET segment_idx = SegmentWithSplit.segment_idx
, segment_split_idx = SegmentWithSplit.segment_split_idx
FROM SegmentWithSplit
WHERE TimelineItem.asset_id = SegmentWithSplit.asset_id;

CREATE TEMP TABLE IF NOT EXISTS SectionMap (
	segment_idx INTEGER NOT NULL UNIQUE
	, segment_len INTEGER NOT NULL
	, section_idx INTEGER NOT NULL
) STRICT;
DELETE FROM SectionMap;

WITH SegmentLen AS (
	SELECT segment_idx
	, COUNT(DISTINCT series_id) + SUM (
		CASE WHEN series_id IS NULL THEN 1 ELSE 0 END
	) AS segment_len
	FROM TimelineItem
	GROUP BY segment_idx
)
, SegmentLenCumul AS (
	SELECT segment_idx
	, segment_len
	, SUM(segment_len) OVER (ORDER BY segment_idx ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS segment_len_cumul
	FROM SegmentLen
)
INSERT INTO SectionMap (
	segment_idx
	, segment_len
	, section_idx
)
SELECT segment_idx
, segment_len
, DENSE_RANK() OVER (ORDER BY segment_len_cumul / 100) AS section_idx
FROM SegmentLenCumul;

INSERT INTO TimelineSection (
	section_idx
	, start_segment
	, end_segment
	, section_len
)
SELECT section_idx
, MIN(segment_idx) AS start_segment
, MAX(segment_idx) + 1 AS end_segment
, SUM(segment_len) AS section_len
FROM SectionMap
GROUP BY section_idx;

UPDATE TimelineItem
SET section_idx = SectionMap.section_idx
FROM SectionMap
WHERE TimelineItem.segment_idx = SectionMap.segment_idx;
