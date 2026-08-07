CREATE TRIGGER Asset_Delete_TimelineDirty
BEFORE DELETE
ON Asset
BEGIN
	UPDATE TimelineItem SET is_dirty = 1 
	WHERE TimelineItem.section_idx = (
		SELECT ti2.section_idx FROM TimelineItem ti2
		WHERE ti2.asset_id = OLD.asset_id
	);
END;

CREATE TRIGGER Asset_Update_TimelineDirty
AFTER UPDATE OF taken_date
, series_id
, is_hidden
ON Asset
BEGIN
	UPDATE TimelineItem SET is_dirty = 1 WHERE TimelineItem.asset_id = NEW.asset_id;
END;

CREATE TRIGGER TimelineGroup_Update_TimelineDirty
AFTER UPDATE OF group_date
ON TimelineGroup
BEGIN
	UPDATE TimelineItem SET is_dirty = 1 WHERE TimelineItem.group_id = NEW.group_id;
END;

CREATE TRIGGER TimelineGroupItem_Insert_TimelineDirty
AFTER INSERT
ON TimelineGroupItem
BEGIN
	UPDATE TimelineItem SET is_dirty = 1 WHERE TimelineItem.asset_id = NEW.asset_id;
END;

CREATE TRIGGER TimelineGroupItem_Delete_TimelineDirty
AFTER DELETE
ON TimelineGroupItem
BEGIN
	UPDATE TimelineItem SET is_dirty = 1 WHERE TimelineItem.asset_id = OLD.asset_id;
END;

CREATE TABLE TimelineItem (
  asset_id INTEGER NOT NULL UNIQUE
  , taken_date INTEGER NOT NULL
  , series_id INTEGER
  , series_date INTEGER
  , group_id INTEGER
  , group_date INTEGER
  , sort_date INTEGER NOT NULL GENERATED ALWAYS AS (IFNULL(group_date, IFNULL(series_date, taken_date))) STORED
  , segment_date TEXT NOT NULL GENERATED ALWAYS AS (date(sort_date / 1000, 'unixepoch')) STORED
  , section_idx INTEGER NOT NULL
  , segment_id INTEGER NOT NULL
  , segment_split_idx INTEGER
  , is_dirty INTEGER NOT NULL DEFAULT 0 CHECK(is_dirty IN (0, 1))
  , FOREIGN KEY (asset_id) REFERENCES Asset(asset_id) ON DELETE CASCADE
  , FOREIGN KEY (series_id) REFERENCES AssetSeries(series_id) ON DELETE CASCADE
  , FOREIGN KEY (group_id) REFERENCES TimelineGroup(timeline_group_id) ON DELETE CASCADE
  , CHECK ((series_id IS NULL) IS (series_date IS NULL))
  , CHECK ((group_id IS NULL) IS (group_date IS NULL))
) STRICT;

CREATE TABLE TimelineSection (
  section_idx INTEGER PRIMARY KEY
  , section_len INTEGER NOT NULL
  , total_width REAL NOT NULL
  , CHECK (0 < section_len)
) STRICT;

CREATE TABLE TimelineSegment (
  segment_id INTEGER PRIMARY KEY
  , section_idx INTEGER NOT NULL
  , FOREIGN KEY (section_idx) REFERENCES TimelineSection(section_idx)
) STRICT;

CREATE TABLE TimelineMonth (
  start_of_month INTEGER NOT NULL
  , num_assets INTEGER NOT NULL CHECK(num_assets > 0)
  , total_width REAL NOT NULL
) STRICT;

CREATE INDEX index_timelineitem_section ON TimelineItem(section_idx);
