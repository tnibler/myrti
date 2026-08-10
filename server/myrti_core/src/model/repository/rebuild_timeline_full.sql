DELETE FROM TimelineItem;
DELETE FROM TimelineSegment;
DELETE FROM TimelineSection;

INSERT INTO RebuildFullTimeline VALUES (1);
