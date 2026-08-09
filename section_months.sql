BEGIN TRANSACTION;
DELETE FROM TimelineMonth;
DELETE FROM TimelineItem;
DELETE FROM TimelineSegment;
DELETE FROM TimelineSection;

WITH RECURSIVE
SeriesDate AS (
	SELECT Asset.*
		, CASE
		WHEN Asset.series_id IS NOT NULL THEN MAX(Asset.taken_date) OVER (PARTITION BY Asset.series_id)  -- most recent date becomes this series' sort date
		ELSE NULL
		END AS series_date
		, CASE 
			-- this assumes all assets of a series are always part of one group, which should be the case
			WHEN group_id IS NOT NULL THEN MAX(Asset.taken_date) OVER (PARTITION BY group_id)
			ELSE NUlL
		END AS group_date
		, TimelineGroupItem.group_id
	FROM Asset
	LEFT JOIN TimelineGroupItem ON Asset.asset_id = TimelineGroupItem.asset_id
)
, AssetGroupDate AS (
	SELECT *
	, COALESCE(COALESCE(group_date, series_date), taken_date) AS sort_date
	FROM SeriesDate
	WHERE is_hidden = 0
	AND (series_id IS NULL OR is_series_selection = 1)
)
, AssetMonthDay AS (
	SELECT asset_id
	, series_id
	, series_date
	, group_id
	, group_date
	, group_id IS NOT NULL 
		AND lag(group_id) OVER (ORDER BY sort_date DESC, taken_date DESC, group_id DESC, asset_id DESC)
		IS NOT group_id AS is_group_start
	, sort_date
	, date(sort_date / 1000, 'unixepoch', 'start of day') AS day
	, taken_date
	FROM AssetGroupDate
)
-- 1 day segment can be split by groups within it. preceding_group_count logic does that
, PrecedingGroupCount AS (
	SELECT asset_id
	, series_id
	, series_date
	, group_id
	, group_date
	, taken_date
	, sort_date
	, day
	, SUM(is_group_start) OVER (
		PARTITION BY day
		ORDER BY sort_date DESC
		ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
	) AS preceding_group_count
	FROM AssetMonthDay
)
, SplitSegmentDates AS (
	SELECT *
	, MAX(sort_date) OVER (
		PARTITION BY day, preceding_group_count
	) AS segment_day
	FROM PrecedingGroupCount
)
, DaySplit AS (
	SELECT *
	, date(segment_day / 1000, 'unixepoch', 'start of month') AS segment_month
	, ROW_NUMBER() OVER (
		PARTITION BY segment_day ORDER BY sort_date DESC, taken_date DESC
	) / 400 AS day_split_idx
	FROM SplitSegmentDates
)
, DayCounts AS (
	SELECT segment_day
	, segment_month
	, day_split_idx
	, group_id
	, COUNT(*) AS day_count
	FROM DaySplit
	GROUP BY group_id, segment_day, day_split_idx
)
, DayOrdered AS (
	SELECT *
	, SUM(day_count) OVER (PARTITION by segment_month) AS month_count
	, ROW_NUMBER() OVER (ORDER BY segment_day DESC, day_split_idx ASC) AS segment_id
	FROM DayCounts ORDER BY segment_day DESC, day_split_idx ASC
)
, Sectioned AS (
	SELECT segment_id
	, segment_month
	, segment_day
	, day_split_idx
	, group_id
	, month_count
	, day_count
	, day_count AS running_count
	, 1 AS section_id
	FROM DayOrdered
	WHERE segment_id = 1

	UNION ALL

	SELECT n.segment_id
	, n.segment_month
	, n.segment_day
	, n.day_split_idx
	, n.group_id
	, n.month_count
	, n.day_count
	, CASE 
		WHEN n.segment_month <> curr.segment_month AND (
			-- merging next month would surpass the limit
             curr.running_count + n.month_count > 800
			-- current has enough for a complete section and next month can fill one by itself
             OR (curr.running_count >= 100 AND n.month_count >= 800)
        ) THEN n.day_count
        WHEN curr.running_count + n.day_count > 800 THEN n.day_count
		ELSE curr.running_count + n.day_count
	END AS running_count
	, CASE 
		WHEN n.segment_month <> curr.segment_month AND (
             curr.running_count + n.month_count > 800
             OR (curr.running_count >= 100 AND n.month_count >= 800)
        ) THEN curr.section_id + 1
        WHEN curr.running_count + n.day_count > 800 THEN curr.section_id + 1
		ELSE curr.section_id
	END AS section_id
	FROM DayOrdered n
	INNER JOIN Sectioned curr ON n.segment_id = curr.segment_id + 1
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
SELECT asset_id
	, taken_date
	, series_id
	, series_date
	, Sectioned.group_id
	, group_date
	, section_id
	, segment_id
	, Sectioned.segment_day
FROM Sectioned INNER JOIN DaySplit
ON Sectioned.group_id IS DaySplit.group_id
AND Sectioned.segment_day = DaySplit.segment_day
AND Sectioned.day_split_idx = DaySplit.day_split_idx
;

WITH SectionLen AS (
	SELECT section_idx
	, COUNT(*) as num_assets
	FROM TimelineItem
	GROUP BY section_idx
)
, SectionWidth AS (
	SELECT section_idx
	, SUM(CAST(AssetFile.width AS REAL) / CAST(AssetFile.height AS REAL)) AS total_width
	FROM TimelineItem INNER JOIN Asset ON TimelineItem.asset_id = Asset.asset_id
	INNER JOIN AssetFile ON Asset.rep_file_id = AssetFile.file_id
	WHERE Asset.series_id IS NULL OR Asset.is_series_selection = 1
	GROUP BY TimelineItem.section_idx
)
INSERT INTO TimelineSection (
	section_idx
	, section_len
	, total_width
)
SELECT SectionLen.section_idx
, SectionLen.num_assets
, SectionWidth.total_width
FROM SectionLen, SectionWidth
WHERE SectionLen.section_idx = SectionWidth.section_idx;
