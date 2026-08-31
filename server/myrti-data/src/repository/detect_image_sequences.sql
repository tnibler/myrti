CREATE INDEX IF NOT EXISTS index_assetfile_merge_fields ON AssetFile(
asset_type
, rtrim(file_path, json_extract(exiftool_output, '$[0].File.FileName'))
, json_extract(exiftool_output, '$[0].EXIF.Make')
, json_extract(exiftool_output, '$[0].EXIF.Model')
, substr(file_path, 1, length(file_path) - length(json_extract(exiftool_output, '$[0].File.FileName')))
);

CREATE INDEX IF NOT EXISTS index_asset_series_id ON Asset(series_id);

DROP TABLE IF EXISTS LocalSequences;
DROP TABLE IF EXISTS GlobalSequences;
CREATE TEMP TABLE IF NOT EXISTS LocalSequences (
	file_id INTEGER PRIMARY KEY NOT NULL
	, asset_id INTEGER NOT NULL
	, asset_type INTEGER NOT NULL
	, file_ext TEXT
	, existing_series_id INTEGER
	, existing_series_type INTEGER
	, existing_series_is_auto INTEGER
	, taken_date INTEGER NOT NULL
	, dirname TEXT NOT NULL
	, exif_make TEXT
	, exif_model TEXT
	, timelapse_idx INTEGER
	, local_seq_id INTEGER
	, is_seq_start INTEGER
) STRICT;
DELETE FROM LocalSequences;

DROP VIEW IF EXISTS FinalSequences;

WITH SeqAssets AS (
	SELECT *
	, CASE
		WHEN timelapse_idx IS NOT LAG(timelapse_idx) OVER w  + 1
			AND LEAD(timelapse_idx) OVER w IS timelapse_idx + 1
			THEN 1
		ELSE 0
	END AS is_seq_start
	, CASE
		WHEN timelapse_idx IS LAG(timelapse_idx) OVER w  + 1
			OR LEAD(timelapse_idx) OVER w IS timelapse_idx + 1
			THEN 1
		ELSE 0
	END AS is_in_seq
	FROM AssetMergeFields af
	INNER JOIN Asset ON af.asset_id = Asset.asset_id
	LEFT JOIN AssetSeries ON
		Asset.series_id = AssetSeries.series_id
	WINDOW w AS (
		PARTITION BY af.asset_type, af.file_ext, af.dirname, af.exif_make, af.exif_model
		ORDER BY Asset.taken_date, af.timelapse_idx
	)
	ORDER BY Asset.taken_date, af.timelapse_idx
)
INSERT INTO LocalSequences (
	file_id
	, asset_id
	, asset_type
	, file_ext
	, existing_series_id
	, existing_series_type
	, existing_series_is_auto
	, taken_date
	, dirname
	, exif_make
	, exif_model
	, timelapse_idx
	, local_seq_id
	, is_seq_start
)
SELECT file_id, asset_id, asset_type, file_ext, series_id, series_type, is_auto, taken_date, dirname, exif_make, exif_model, timelapse_idx
, CASE WHEN is_in_seq IS 1
	THEN (SUM(COALESCE(is_seq_start, 0)) OVER (
		PARTITION BY asset_type, file_ext, dirname, exif_make, exif_model
		ORDER BY taken_date, timelapse_idx
		ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW))
	ELSE NULL
END AS local_seq_id
, COALESCE(is_seq_start, 0)
FROM SeqAssets;

CREATE TEMP TABLE IF NOT EXISTS GlobalSequences (
	asset_id INTEGER NOT NULL
	, final_series_id INTEGER
	, is_seq_start INTEGER
	, existing_series_id INTEGER
	, existing_series_is_auto INTEGER
	) STRICT;


WITH GroupedLocalSeqs AS (
	SELECT *
	, MIN(taken_date) AS seq_start_date
	, COUNT(*) AS seq_len
	FROM LocalSequences WHERE local_seq_id <> 0
	GROUP BY asset_type, file_ext, dirname, exif_make, exif_model, local_seq_id
	-- Early exit if local_seq_id perfectly maps to an existing_series_id
	HAVING COUNT(DISTINCT existing_series_id) <> COUNT(DISTINCT local_seq_id) OR COUNT(COALESCE(existing_series_id, 0)) <> COUNT(existing_series_id)
)
, WithGlobalSeqId AS (
	SELECT *
	, DENSE_RANK() OVER (
		ORDER BY asset_type, file_ext, dirname, exif_make, exif_model, seq_start_date
	) AS seq_id
	FROM GroupedLocalSeqs
)
, UniqueSeqIds AS (
	SELECT asset_type, file_ext, dirname, exif_make, exif_model, local_seq_id, seq_start_date, is_seq_start, seq_len
	, seq_id + (SELECT COALESCE(MAX(series_id), 0) FROM AssetSeries) AS global_series_id
	FROM WithGlobalSeqId
)
, GlobalLocalMap AS (
	SELECT gs.global_series_id
		, ls.local_seq_id
		, ls.is_seq_start
		, ls.asset_id
		, ls.taken_date
		, ls.timelapse_idx
		, ls.existing_series_id
		, ls.existing_series_type
		, ls.existing_series_is_auto
		FROM UniqueSeqIds gs INNER JOIN LocalSequences ls
		ON gs.local_seq_id=ls.local_seq_id
		AND gs.asset_type=ls.asset_type
		AND gs.file_ext=ls.file_ext
		AND gs.dirname=ls.dirname
		AND gs.exif_make=ls.exif_make
		AND gs.exif_model=ls.exif_model
)
, AggSeqs AS (
	SELECT
		global_series_id
		, json_group_array(asset_id ORDER BY taken_date, timelapse_idx) AS asset_ids
		, MIN(existing_series_id) AS existing_series_id
		, MIN(existing_series_is_auto) AS existing_series_is_auto
		FROM GlobalLocalMap
		GROUP BY global_series_id
)
, DedupedSeqs AS (
	SELECT MIN(global_series_id) AS global_series_id,
	MIN(existing_series_id) AS existing_series_id,
	MIN(existing_series_is_auto) AS existing_series_is_auto
	, asset_ids
	FROM AggSeqs
	GROUP BY asset_ids
	HAVING MIN(existing_series_is_auto) IS NOT 0
)
INSERT INTO GlobalSequences(asset_id, final_series_id, is_seq_start, existing_series_id, existing_series_is_auto)
SELECT asset_id
, GlobalLocalMap.global_series_id
, GlobalLocalMap.is_seq_start
, DedupedSeqs.existing_series_id
, DedupedSeqs.existing_series_is_auto
FROM GlobalLocalMap
INNER JOIN DedupedSeqs ON 
GlobalLocalMap.global_series_id = DedupedSeqs.global_series_id;

WITH AnyExistingSeries AS (
	SELECT final_series_id, MAX(existing_series_id) as existing_series_id
	FROM GlobalSequences
	GROUP BY final_series_id
)
INSERT INTO AssetSeries(series_id, series_type, is_auto)
SELECT DISTINCT final_series_id, 2, 1
FROM GlobalSequences
WHERE (SELECT AnyExistingSeries.existing_series_id FROM AnyExistingSeries WHERE AnyExistingSeries.final_series_id = GlobalSequences.final_series_id) IS NULL;

WITH cte AS (
	SELECT asset_id, final_series_id, is_seq_start AS is_series_selection
	FROM GlobalSequences
	WHERE existing_series_id IS NULL
)
UPDATE Asset
SET
    series_id = cte.final_series_id,
    is_series_selection = cte.is_series_selection
FROM cte
WHERE Asset.asset_id = cte.asset_id;

-- Merge existing auto series
WITH cte AS (
	SELECT asset_id
	, MIN(existing_series_id) OVER (PARTITION BY final_series_id) AS final_series_id
	, is_seq_start AS is_series_selection
	FROM GlobalSequences
	WHERE existing_series_id IS NOT NULL
)
UPDATE Asset
SET
    series_id = cte.final_series_id,
    is_series_selection = cte.is_series_selection
FROM cte
WHERE Asset.asset_id = cte.asset_id;

WITH RetainedSeries AS (
	SELECT existing_series_id, MIN(existing_series_id) OVER (PARTITION BY final_series_id) AS retained_series
	FROM GlobalSequences
	WHERE existing_series_id IS NOT NULL
)
DELETE FROM AssetSeries
WHERE AssetSeries.is_auto = 1
AND AssetSeries.series_type = 2
AND AssetSeries.series_id <> (SELECT retained_series FROM RetainedSeries WHERE existing_series_id = AssetSeries.series_id);
