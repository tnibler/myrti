DELETE FROM TimelineItem;
DELETE FROM TimelineSegment;
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
	, segment_id
	, segment_date
)
SELECT Asset.asset_id
, Asset.taken_date
, SeriesDate.series_id
, SeriesDate.series_date
, group_id
, group_date
, -1
, -1
, ''
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
	, group_id IS NOT NULL AND lag(group_id) OVER w IS NOT group_id AS is_group_start
	, series_id
	, series_id IS NOT NULL AND lag(series_id) OVER w IS NOT series_id AS is_series_start
	, date(COALESCE(group_date, taken_date) / 1000, 'unixepoch') AS group_or_taken_day
	, group_date
	FROM TimelineItem
	WINDOW w AS (
		ORDER BY segment_date DESC, taken_date DESC, group_id DESC, asset_id DESC
		ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
	)
)
-- 1 day segment can be split by groups within it. preceding_group_count logic does that
, PrecedingGroupCount AS (
	SELECT *
	, SUM(is_group_start) OVER (
		PARTITION BY group_or_taken_day
		ORDER BY COALESCE(group_date, taken_date) DESC
		ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
	) AS preceding_group_count
	FROM NewSeriesStart
)
, SplitSegmentDates AS (
	SELECT *
	, MAX(COALESCE(group_date, taken_date)) OVER (
		PARTITION BY group_or_taken_day, preceding_group_count
	) AS segment_date
	FROM PrecedingGroupCount
)
, RawSegmentRank AS (
	SELECT *
	, DENSE_RANK() OVER (
		ORDER BY preceding_group_count DESC, segment_date DESC
		, IFNULL(group_id, 0) DESC
	)
	AS raw_segment_id
	, SUM(is_series_start) OVER w + SUM (CASE WHEN series_id IS NULL THEN 1 ELSE 0 END) OVER w AS cumul_segment_len
	FROM SplitSegmentDates
	WINDOW w AS (
		PARTITION BY preceding_group_count, segment_date , IFNULL(group_id, 0) -- must be same as dense_rank() above
		ORDER BY segment_date DESC, taken_date DESC, asset_id DESC
		ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
	)
)
, SegmentRank AS (
	SELECT *
	, cumul_segment_len / 100 AS segment_split_idx
	, segment_date
	, DENSE_RANK() OVER (
		ORDER BY segment_date DESC
		, IFNULL(group_id, 0) DESC
		, cumul_segment_len / 100 ASC
	)
	AS segment_id
	FROM RawSegmentRank
)
, SegmentWithSplit AS (
	SELECT asset_id
	, segment_id
	, segment_date
	, CASE WHEN (MAX(segment_split_idx) OVER (PARTITION BY raw_segment_id)) <> 0
		THEN segment_split_idx
		ELSE NULL
	END AS segment_split_idx
	FROM SegmentRank
)
UPDATE TimelineItem
SET segment_id = SegmentWithSplit.segment_id
, segment_split_idx = SegmentWithSplit.segment_split_idx
, segment_date = SegmentWithSplit.segment_date
FROM SegmentWithSplit
WHERE TimelineItem.asset_id = SegmentWithSplit.asset_id;

CREATE TEMP TABLE IF NOT EXISTS SectionMap (
	segment_id INTEGER NOT NULL UNIQUE
	, segment_len INTEGER NOT NULL
	, section_idx INTEGER NOT NULL
) STRICT;
DELETE FROM SectionMap;

WITH SegmentLen AS (
	SELECT segment_id
	, COUNT(DISTINCT series_id) + SUM (
		CASE WHEN series_id IS NULL THEN 1 ELSE 0 END
	) AS segment_len
	FROM TimelineItem
	GROUP BY segment_id
)
, SegmentLenCumul AS (
	SELECT segment_id
	, segment_len
	, SUM(segment_len) OVER (ORDER BY segment_id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS segment_len_cumul
	FROM SegmentLen
)
INSERT INTO SectionMap (
	segment_id
	, segment_len
	, section_idx
)
SELECT segment_id
, segment_len
, DENSE_RANK() OVER (ORDER BY segment_len_cumul / 100) AS section_idx
FROM SegmentLenCumul;

UPDATE TimelineItem
SET section_idx = SectionMap.section_idx
FROM SectionMap
WHERE TimelineItem.segment_id = SectionMap.segment_id;

INSERT INTO TimelineSection (
	section_idx
	, section_len
	, total_width
)
SELECT SectionMap.section_idx
, SUM(SectionMap.segment_len)
, SUM(CAST(AssetFile.width AS REAL) / CAST(AssetFile.height AS REAL))
FROM SectionMap INNER JOIN TimelineItem ON SectionMap.section_idx = TimelineItem.section_idx
INNER JOIN Asset ON TimelineItem.asset_id = Asset.asset_id
INNER JOIN AssetFile ON Asset.rep_file_id = AssetFile.file_id
WHERE Asset.series_id IS NULL OR Asset.is_series_selection = 1
GROUP BY SectionMap.section_idx;
