DELETE FROM TimelineItem;
DELETE FROM TimelineMonth;
DELETE FROM TimelineSection;

INSERT INTO RebuildFullTimeline VALUES (1);
