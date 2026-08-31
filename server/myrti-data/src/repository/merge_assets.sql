DROP VIEW IF EXISTS AssetMergeFields;
DROP VIEW IF EXISTS MergeMap;

CREATE TEMP VIEW IF NOT EXISTS AssetMergeFields AS
WITH cte AS (
	SELECT AssetFile.*
	, json_extract(AssetFile.exiftool_output, '$[0].File.FileName') AS filename
	, json_extract(AssetFile.exiftool_output, '$[0].File.FileTypeExtension') AS file_ext
	FROM AssetFile
)
SELECT cte.*
, substr(file_path, 1, length(file_path) - length(filename)) AS dirname
, CASE 
         WHEN filename LIKE '%.' || file_ext
         THEN substr(filename, 1, length(filename) - length(file_ext) - 1) 
         ELSE filename 
       END AS file_stem
, json_extract(exiftool_output, '$[0].EXIF.Make') AS exif_make
, json_extract(exiftool_output, '$[0].EXIF.Model') AS exif_model
, json_extract(exiftool_output, '$[0].MakerNotes.TimeLapseShotNumber') as timelapse_idx
, json_extract(exiftool_output, '$[0].MakerNotes.SequenceNumber') as sequence_idx
, json_extract(exiftool_output, '$[0].EXIF.DateTimeOriginal') as date1
, json_extract(exiftool_output, '$[0].EXIF.CreateDate') as date2
FROM cte;

CREATE TEMP VIEW IF NOT EXISTS MergeMap AS
SELECT AssetMergeFields.*
, ROW_NUMBER() OVER w AS group_rank
  -- having ORDER BY in the window switches to cumulative aggregates so specify to calculate over the entire group
, COUNT(*) OVER (w ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) AS merge_count
, FIRST_VALUE(asset_id) OVER (w ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) AS merge_asset_id
FROM AssetMergeFields
WINDOW w AS (
	PARTITION BY asset_type, dirname, file_stem, exif_make, exif_model, timelapse_idx, sequence_idx, date1, date2
	ORDER BY
	lower(file_type) IN ('jpg', 'jpeg') DESC
	, lower(file_type) IN ('avif', 'webp') DESC
	, asset_id
);


-- Be sure to point any references to merged assets to the new canonical asset.
UPDATE TimelineGroupItem SET asset_id = MergeMap.merge_asset_id
FROM MergeMap
WHERE MergeMap.asset_id = TimelineGroupItem.asset_id AND MergeMap.group_rank > 1;

UPDATE AlbumItem SET asset_id = MergeMap.merge_asset_id
FROM MergeMap
WHERE MergeMap.asset_id = AlbumItem.asset_id AND MergeMap.group_rank > 1;

DELETE FROM Asset WHERE asset_id NOT IN (SELECT merge_asset_id FROM MergeMap);

UPDATE AssetFile
SET asset_id = MergeMap.merge_asset_id
, merge_reason = 1
FROM MergeMap
WHERE MergeMap.merge_count > 1 
AND MergeMap.file_id = AssetFile.file_id 
-- keep merge_reason as NULL for the original AssetFile row. no specific reason rn just seems correct
AND AssetFile.asset_id <> MergeMap.merge_asset_id;


UPDATE Asset SET rep_file_id = MergeMap.file_id
FROM MergeMap
WHERE MergeMap.merge_asset_id = Asset.asset_id AND MergeMap.group_rank = 1 AND MergeMap.merge_count > 1;
