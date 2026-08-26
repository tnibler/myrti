CREATE TEMP VIEW IF NOT EXISTS MinSectionLen AS SELECT 400 AS min_section_len;
CREATE TEMP VIEW IF NOT EXISTS MaxSectionLen AS SELECT 1000 AS max_section_len;

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
	WHERE Asset.asset_id IN (SELECT asset_id FROM BuildTimelineStaging)
	OR (EXISTS (SELECT * FROM RebuildFullTimeline) AND Asset.is_hidden = 0)
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
             curr.running_count + n.month_count > (SELECT * FROM MaxSectionLen)
			-- current has enough for a complete section and next month can fill one by itself
             OR (curr.running_count >= (SELECT * FROM MinSectionLen) AND n.month_count >= (SELECT * FROM MaxSectionLen))
        ) THEN n.day_count
        WHEN curr.running_count + n.day_count > (SELECT * FROM MaxSectionLen) THEN n.day_count
		ELSE curr.running_count + n.day_count
	END AS running_count
	, CASE 
		WHEN n.segment_month <> curr.segment_month AND (
             curr.running_count + n.month_count > (SELECT * FROM MaxSectionLen)
             OR (curr.running_count >= (SELECT * FROM MinSectionLen) AND n.month_count >= (SELECT * FROM MaxSectionLen))
        ) THEN curr.section_id + 1
        WHEN curr.running_count + n.day_count > (SELECT * FROM MaxSectionLen) THEN curr.section_id + 1
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
	, segment_split_idx
)
SELECT asset_id
	, taken_date
	, series_id
	, series_date
	, Sectioned.group_id
	, group_date
	, section_id + COALESCE((SELECT MAX(section_idx) FROM BuildTimelineStaging), 1) - 1
	, segment_id + COALESCE((SELECT MAX(TimelineItem.segment_id) FROM TimelineItem), 0)
	, Sectioned.segment_day
	, CASE 
		WHEN (MAX(DaySplit.day_split_idx) OVER (PARTITION BY DaySplit.segment_day)) <> 0
			THEN DaySplit.day_split_idx
		ELSE NULL
	END
FROM Sectioned INNER JOIN DaySplit
ON Sectioned.group_id IS DaySplit.group_id
AND Sectioned.segment_day = DaySplit.segment_day
AND Sectioned.day_split_idx = DaySplit.day_split_idx
;

-- Add difference between highest section_idx of old and new TimelineItems
-- to all items after the recomputed sections
CREATE TEMP TABLE IF NOT EXISTS ShiftedSections (
	old_idx INTEGER NOT NULL UNIQUE
	, new_idx INTEGER NOT NULl UNIQUE
) STRICT;
DELETE FROM ShiftedSections;

WITH MaxRemovedSection AS (
	SELECT MAX(section_idx) AS old_max FROM BuildTimelineStaging
)
INSERT INTO ShiftedSections(old_idx, new_idx)
SELECT TimelineSection.section_idx
, TimelineSection.section_idx + (
	SELECT MAX(ti.section_idx) - (SELECT old_max FROM MaxRemovedSection) AS shift_amount
	FROM TimelineItem ti
	WHERE ti.asset_id IN (SELECT asset_id FROM BuildTimelineStaging)
)
FROM TimelineSection
WHERE section_idx NOT IN (SELECT section_idx FROM BuildTimelineStaging)
AND section_idx > (SELECT old_max FROM MaxRemovedSection)
AND NOT EXISTS (SELECT * FROM RebuildFullTimeline);


UPDATE TimelineItem
SET section_idx = (SELECT new_idx FROM ShiftedSections WHERE ShiftedSections.old_idx = section_idx)
WHERE section_idx IN (SELECT old_idx FROM ShiftedSections);
UPDATE TimelineSection
SET section_idx = (SELECT new_idx FROM ShiftedSections WHERE ShiftedSections.old_idx = section_idx)
WHERE section_idx IN (SELECT old_idx FROM ShiftedSections);

WITH SectionLen AS (
	SELECT section_idx
	, COUNT(*) as num_assets
	FROM TimelineItem
	WHERE TimelineItem.asset_id IN (SELECT asset_id FROM BuildTimelineStaging)
	OR EXISTS (SELECT * FROM RebuildFullTimeline)
	GROUP BY section_idx
)
, SectionWidth AS (
	SELECT section_idx
	, SUM(CAST(AssetFile.width AS REAL) / CAST(AssetFile.height AS REAL)) AS total_width
	FROM TimelineItem INNER JOIN Asset ON TimelineItem.asset_id = Asset.asset_id
	INNER JOIN AssetFile ON Asset.rep_file_id = AssetFile.file_id
	WHERE (Asset.asset_id IN (SELECT asset_id FROM BuildTimelineStaging)
		OR EXISTS (SELECT * FROM RebuildFullTimeline))
	AND (Asset.series_id IS NULL OR Asset.is_series_selection = 1)
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

-- Insert assets in series that are not in the series selection, which were filtered out at the very start.
-- Do this after sections and widths are computed since these do not appear in the grid.
WITH SeriesSelIndex AS (
	-- associate each asset with the selection asset its associated with
	SELECT Asset.asset_id
	, Asset.series_id
	, Asset.is_series_selection
	, MAX(Asset.taken_date) OVER (PARTITION BY Asset.series_id) AS series_date
	, COALESCE(SUM(Asset.is_series_selection) OVER (
		PARTITION BY Asset.series_id
		ORDER BY Asset.taken_date
		ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
	), 0) AS selection_index
	, SUM(Asset.is_series_selection) OVER (PARTITION BY Asset.series_id) AS selection_count
	FROM Asset
	WHERE Asset.series_id IS NOT NULL
	AND (
		Asset.asset_id IN (SELECT asset_id FROM BuildTimelineStaging)
		OR (EXISTS (SELECT * FROM RebuildFullTimeline) AND Asset.is_hidden = 0)
	)
	WINDOW w AS (PARTITION BY Asset.series_id ORDER BY Asset.taken_date)
)
, MatchSelectionIndex AS (
	SELECT not_sel.asset_id AS asset_id
	, sel.asset_id AS sel_asset_id
	FROM SeriesSelIndex sel, SeriesSelIndex not_sel
	WHERE sel.is_series_selection = 1 AND not_sel.is_series_selection = 0
	AND sel.series_id = not_sel.series_id
	AND CASE
		-- the tail is special, there's no selection asset after the last one so include them in the last group
		WHEN not_sel.selection_index = not_sel.selection_count THEN not_sel.selection_index = sel.selection_index + 1
		ELSE sel.selection_index = not_sel.selection_index
	END
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
	, segment_split_idx
)
SELECT Asset.asset_id
	, Asset.taken_date
	, Asset.series_id
	, ti.series_date
	, ti.group_id
	, ti.group_date -- series must always belong to the same group, so copy group info
	, ti.section_idx
	, ti.segment_id
	, ti.segment_date
	, ti.segment_split_idx
FROM MatchSelectionIndex
INNER JOIN Asset ON Asset.asset_id = MatchSelectionIndex.asset_id
INNER JOIN TimelineItem ti ON ti.asset_id = MatchSelectionIndex.sel_asset_id
;

