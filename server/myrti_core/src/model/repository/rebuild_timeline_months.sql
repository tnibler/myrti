DELETE FROM TimelineMonth;

WITH AssetMonth AS (
	SELECT unixepoch(date(Asset.taken_date / 1000, 'unixepoch', 'start of month')) AS start_of_month
	, CAST(AssetFile.width AS REAL) / CAST(AssetFile.height as REAL) AS normalized_width
	FROM Asset INNER JOIN AssetFile
	ON Asset.rep_file_id = AssetFile.file_id
)
INSERT INTO TimelineMonth(start_of_month, num_assets, total_width)
SELECT start_of_month
	, COUNT(*)
	, SUM(normalized_width)
FROM AssetMonth 
GROUP BY start_of_month;
