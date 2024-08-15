WITH
-- AssetSeries with date of its oldest Asset  
series AS (
	SELECT 
		AssetSeries.*, 
		MIN(Asset.taken_date) AS series_date,
		COUNT(Asset.asset_id) AS series_len
	FROM Asset INNER JOIN AssetSeries
	ON Asset.series_id = AssetSeries.series_id 
	GROUP BY AssetSeries.series_id
),
timeline_sort AS (
	SELECT 
	Asset.asset_id AS asset_id,
	Asset.taken_date AS taken_date,
	series_date,
	series_len,
	Asset.series_id AS series_id,
	group_date,
	group_id,
	-- sort_date is sorting key of highest level grouping (group > series > nothing)
	IFNULL(group_date, IFNULL(series_date, taken_date)) AS sort_date
	FROM 
	Asset LEFT JOIN (
		SELECT TimelineGroup.timeline_group_id AS group_id, TimelineGroup.display_date AS group_date, TimelineGroupItem.group_id, TimelineGroupItem.asset_id
		FROM TimelineGroup INNER JOIN TimelineGroupItem ON TimelineGroup.timeline_group_id = TimelineGroupItem.group_id
	) tgi
	ON Asset.asset_id = tgi.asset_id
	LEFT JOIN series
	ON Asset.series_id = series.series_id
	WHERE Asset.is_hidden = 0
	ORDER BY sort_date DESC, series_date DESC, taken_date DESC, 
	-- fallback sort by id to get stable results
	series_id, group_id, asset_id
),
segment_idx AS (
	SELECT *,
	CASE WHEN group_id IS NULL THEN date(sort_date / 1000, 'unixepoch') ELSE NULL END AS sort_date_day,
	DENSE_RANK() OVER 
	(
	  ORDER BY 
	  date(sort_date / 1000, 'unixepoch') DESC, -- we store milliseconds, sqlite uses seconds
	  IFNULL(group_id, 0) DESC -- groups first in case of equal dates, themselves sorted by id for stability
	) AS segment_idx
	FROM timeline_sort
)
SELECT
asset_id,
taken_date,
series_id,
series_date,
series_len,
group_id,
group_date,
sort_date,
sort_date_day, -- TODO rename to sort_date_day
segment_idx
FROM segment_idx
