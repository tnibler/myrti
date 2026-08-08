DELETE FROM TimelineMonth;

CREATE TEMP TABLE IF NOT EXISTS DirtySections (
	section_idx INTEGER NOT NULL UNIQUE
	, min_date INTEGER
	, max_date INTEGER
) STRICT;

DELETE FROM DirtySections;

INSERT INTO DirtySections
SELECT DISTINCT section_idx, NULL, NULL FROM TimelineItem WHERE is_dirty = 1;

INSERT INTO DirtySections
SELECT DISTINCT TimelineItem.section_idx, NULL, NULL
FROM TimelineItem 
WHERE TimelineItem.series_id IN (
	SELECT DISTINCT ti2.series_id 
	FROM TimelineItem ti2 INNER JOIN DirtySections
	ON ti2.section_idx = DirtySections.section_idx
)
AND TimelineItem.section_idx NOT IN (SELECT section_idx FROM DirtySections)
UNION
SELECT DISTINCT TimelineItem.section_idx, NULL, NULL
FROM TimelineItem 
WHERE TimelineItem.group_id IN (
	SELECT DISTINCT ti2.group_id
	FROM TimelineItem ti2 INNER JOIN DirtySections
	ON ti2.section_idx = DirtySections.section_idx
)
AND TimelineItem.section_idx NOT IN (SELECT section_idx FROM DirtySections);

WITH SectionDates AS (
	SELECT DirtySections.section_idx
	, MIN(TimelineItem.sort_date) AS min_date
	, MAX(TimelineItem.sort_date) AS max_date
	FROM TimelineItem INNER JOIN DirtySections
	ON TimelineItem.section_idx = DirtySections.section_idx
)
UPDATE DirtySections
SET min_date = SectionDates.min_date
, max_date = SectionDates.max_date
FROM SectionDates
WHERE SectionDates.section_idx = DirtySections.section_idx;

UPDATE TimelineItem
SET is_dirty = 1
WHERE section_idx IN (SELECT section_idx FROM DirtySections);

DELETE FROM TimelineItem
WHERE (SELECT Asset.is_hidden FROM Asset WHERE Asset.asset_id = TimelineItem.asset_id) = 1;

DELETE FROM TimelineSection
WHERE TimelineSection.section_idx IN (SELECT section_idx FROM DirtySections);

WITH
SeriesDate AS (
	SELECT Asset.*
		, CASE 
			WHEN Asset.series_id IS NULL THEN NULL
			ELSE MAX(Asset.taken_date) OVER (PARTITION BY AssetSeries.series_id) -- most recent date becomes this series' sort date
		END AS series_date
		, tgi.group_id AS group_id
		, CASE WHEN group_id IS NOT NULL THEN MAX(Asset.taken_date) OVER (PARTITION BY group_id) ELSE NULL END AS group_date
	FROM Asset LEFT JOIN AssetSeries
	ON Asset.series_id = AssetSeries.series_id
	LEFT JOIN (
		SELECT TimelineGroupItem.asset_id
		, TimelineGroup.timeline_group_id AS group_id
		FROM TimelineGroup INNER JOIN TimelineGroupItem
		ON TimelineGroup.timeline_group_id = TimelineGroupItem.group_id
	) tgi ON Asset.asset_id = tgi.asset_id
)
UPDATE TimelineItem
SET taken_date = SeriesDate.taken_date
, series_id = SeriesDate.series_id
, series_date = SeriesDate.series_date
, group_id = SeriesDate.group_id
, group_date = SeriesDate.group_date
FROM SeriesDate
WHERE SeriesDate.asset_id = TimelineItem.asset_id
AND TimelineItem.is_dirty = 1;

WITH 
DirtyTimelineItem AS (
	SELECT * FROM TimelineItem WHERE is_dirty = 1
)
, NewSeriesStart AS (
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
, is_dirty = 0
FROM SegmentWithSplit
WHERE TimelineItem.asset_id = SegmentWithSplit.asset_id;

UPDATE TimelineItem
SET section_idx = DirtySections.section_idx
FROM DirtySections
WHERE DirtySections.min_date <= TimelineItem.sort_date
AND TimelineItem.sort_date <= DirtySections.max_date;

INSERT INTO TimelineSection (
	section_idx
	, section_len
	, total_width
)
SELECT section_idx
, COUNT(*) as num_assets
, SUM(CAST(AssetFile.width AS REAL) / CAST(AssetFile.height AS REAL)) AS total_width
FROM TimelineItem INNER JOIN Asset ON TimelineItem.asset_id = Asset.asset_id
INNER JOIN AssetFile ON Asset.rep_file_id = AssetFile.file_id
WHERE (Asset.series_id IS NULL OR Asset.is_series_selection = 1)
AND TimelineItem.section_idx IN (SELECT section_idx FROM DirtySections)
GROUP BY TimelineItem.section_idx;
