DELETE FROM TimelineMonth;

CREATE TEMP TABLE IF NOT EXISTS RebuildFullTimeline (
	rebuild_full INTEGER NOT NULL UNIQUE CHECK ( rebuild_full IN (NULL, 1) )
) STRICT;
DELETE FROM RebuildFullTimeline;

CREATE TEMP TABLE IF NOT EXISTS BuildTimelineStaging (
	asset_id INTEGER NOT NULL UNIQUE
	, section_idx INTEGER
) STRICT;
DELETE FROM BuildTimelineStaging;
