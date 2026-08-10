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

INSERT INTO BuildTimelineStaging(asset_id, section_idx)
SELECT TimelineItem.asset_id, TimelineItem.section_idx
FROM TimelineItem
WHERE TimelineItem.section_idx IN (SELECT section_idx FROM DirtySections);

INSERT INTO BuildTimelineStaging(asset_id, section_idx)
SELECT TimelineItem.asset_id, TimelineItem.section_idx
FROM TimelineItem INNER JOIN Asset ON TimelineItem.asset_id = Asset.asset_id
AND Asset.is_hidden = 1;

DELETE FROM TimelineItem
WHERE TimelineItem.asset_id IN (SELECT asset_id FROM BuildTimelineStaging);

DELETE FROM TimelineSection
WHERE TimelineSection.section_idx IN (SELECT section_idx FROM DirtySections);
