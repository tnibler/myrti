WITH AssetMonth AS (
	SELECT Asset.asset_id
	, unixepoch(date(Asset.taken_date / 1000, 'unixepoch', 'start of month')) AS start_of_month
	, CAST(AssetFile.width AS REAL) / CAST(AssetFile.height as REAL) AS normalized_width
	FROM Asset INNER JOIN AssetFile
	ON Asset.rep_file_id = AssetFile.file_id
	WHERE Asset.is_hidden = 0
	AND (Asset.series_id IS NULL OR Asset.is_series_selection = 1)
)
INSERT INTO TimelineMonth(
	start_of_month
	, section_idx
	, num_assets
	, total_width
)
SELECT start_of_month
	, TimelineItem.section_idx
	, COUNT(*)
	, SUM(normalized_width)
FROM AssetMonth INNER JOIN TimelineItem on AssetMonth.asset_id = TimelineItem.asset_id
GROUP BY section_idx, start_of_month;
