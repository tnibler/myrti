CREATE TABLE AssetRootDir (
  asset_root_dir_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , path TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE DataDir (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , path TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE AssetSeries (
  series_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  -- 0 unknown
  -- 1 burst shot
  -- 2 timelapse
  , series_type INTEGER NOT NULL
  , is_auto INTEGER NOT NULL
  , CONSTRAINT boolean_is_auto CHECK (is_auto IN (0, 1))
) STRICT;

CREATE TABLE Asset (
  asset_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  -- 1=Image, 2=Video
  , asset_type INTEGER NOT NULL
    CONSTRAINT enum_asset_type CHECK (asset_type IN (1, 2))
  , rep_file_id INTEGER NOT NULL UNIQUE
  , is_hidden INTEGER NOT NULL
    CONSTRAINT enum_is_hidden CHECK (is_hidden IN (0, 1))

  -- UTC timestamp in milliseconds since UNIX epoch
  , taken_date INTEGER NOT NULL
  -- "+03:00"
  , timezone_offset TEXT
  , timezone_info INTEGER NOT NULL

  , series_id INTEGER
  , is_series_selection INTEGER
    CONSTRAINT opt_cols_series CHECK ((series_id IS NULL) IS (is_series_selection IS NULL) AND is_series_selection IN (0, 1, NULL))

  -- latitude and longitude are stored multipled by 10e8
  , gps_latitude INTEGER
  , gps_longitude INTEGER

  , UNIQUE(asset_id, asset_type)

  -- timezone_offset NULL is only valid for timezone_info=UtcCertain, and NoTimestamp I guess?
  , CONSTRAINT enum_timezone_info CHECK (timezone_info IN (1, 2, 3, 4, 5, 6) AND (timezone_info IN (2, 6) OR timezone_offset IS NOT NULL))
  , FOREIGN KEY (rep_file_id, asset_type) REFERENCES AssetFile(file_id, asset_type)
  , UNIQUE(rep_file_id)
  , UNIQUE(rep_file_id, asset_type)
  , FOREIGN KEY (series_id) REFERENCES AssetSeries(series_id)
  , CONSTRAINT opt_cols_gps CHECK((gps_latitude IS NULL AND gps_longitude IS NULL) OR (gps_latitude IS NOT NULL AND gps_longitude IS NOT NULL))
);

CREATE TABLE AssetFile (
  file_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  -- 1=Image, 2=Video
  , asset_type INTEGER NOT NULL
    CONSTRAINT enum_asset_type CHECK (asset_type IN (1, 2))
  , root_dir_id INTEGER NOT NULL
  , asset_id INTEGER NOT NULL
  , merge_reason INTEGER DEFAULT NULL
  , file_path TEXT NOT NULL
  , file_type TEXT NOT NULL -- FileType from exiftool
  , hash BLOB UNIQUE
  -- UTC timestamp in milliseconds since UNIX epoch
  , added_at INTEGER NOT NULL
  -- width and height of the image/video as it is displayed, all metadata taken into account
  , width INTEGER NOT NULL
  , height INTEGER NOT NULL
  -- rotation correction applied after exif/metadata rotation if that's still wrong
  , rotation_correction INTEGER NOT NULL DEFAULT 0
    CONSTRAINT enum_rotation_correction CHECK(rotation_correction IN (0, 1, 2, 3))
  -- 0: Nothing, 1: Horizontal, 2: Vertical, 3: Both
  , mirror_correction INTEGER NOT NULL DEFAULT 0
    CONSTRAINT enum_mirror_correction CHECK(mirror_correction IN (0, 1, 2, 3))
  , thumb_hash TEXT

  -- Metadata
  -- exiftool -j -g
  , exiftool_output BLOB NOT NULL
  , file_name TEXT GENERATED ALWAYS AS (json_extract(exiftool_output, '$[0].File.FileName')) VIRTUAL
  , dir_name TEXT GENERATED ALWAYS AS (json_extract(exiftool_output, '$[0].File.Directory')) VIRTUAL
  , file_ext TEXT GENERATED ALWAYS AS (json_extract(exiftool_output, '$[0].File.FileTypeExtension')) VIRTUAL
  , file_stem TEXT GENERATED ALWAYS AS (
      CASE
         WHEN file_name LIKE '%.' || file_ext
         THEN substr(file_name, 1, length(file_name) - length(file_ext) - 1)
         ELSE file_name
       END
    ) STORED

  , FOREIGN KEY (asset_id, asset_type) REFERENCES Asset(asset_id, asset_type) DEFERRABLE INITIALLY DEFERRED
  , FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(asset_root_dir_id)
  , UNIQUE(root_dir_id, file_path)
  , CONSTRAINT unique_hash UNIQUE(hash)
  -- FKs in Video/ImageAsset need to reference a candidate key in Asset.
  -- Any set of columns (asset_id, ...) obviously fulfills that, but sqlite doesn't know so it needs an explicit index
  , CONSTRAINT unique_file_id_asset_id UNIQUE(file_id, asset_id)
  , CONSTRAINT unique_file_id_asset_type UNIQUE(file_id, asset_type)
) STRICT;

CREATE TABLE VideoFile (
  file_id INTEGER PRIMARY KEY NOT NULL
  , asset_type INTEGER NOT NULL DEFAULT 2
    CONSTRAINT const_asset_type CHECK (asset_type = 2)
  , ffprobe_output BLOB NOT NULL
  , video_codec_name TEXT NOT NULL
  , video_bitrate INTEGER
  , video_duration_ms INTEGER
  , audio_codec_name TEXT
  , frame_rate_num INTEGER
    CONSTRAINT valid_frame_rate_num CHECK(frame_rate_num IS NULL OR frame_rate_num > 0)
  , frame_rate_denom INTEGER
    CONSTRAINT valid_frame_rate_denom CHECK(frame_rate_denom IS NULL OR frame_rate_denom > 0)

  -- NULL: unknown
  -- 0: not streamable
  -- 1: video
  -- 2: audio
  -- 3: video+audio
  , original_streaming INTEGER
    CONSTRAINT enum_original_streaming CHECK(original_streaming IN (0, 1, 2, 3))
  , ghi_disabled INTEGER NOT NULL
    CONSTRAINT bool_ghi_disabled CHECK(ghi_disabled IN (0, 1))
  -- NULL: unknown
  -- 0: GHI index not created or DASH segmenter not run
  -- 1: GHI created
  -- 2: DASH segmented
  , original_streaming_state INTEGER
    CONSTRAINT enum_original_streaming_state CHECK(original_streaming_state IN (NULL, 0, 1))
  -- interval in milliseconds
  , max_iframe_interval INTEGER

  , CONSTRAINT opt_cols_frame_rate CHECK((frame_rate_num IS NULL) = (frame_rate_denom IS NULL))
  , CONSTRAINT valid_original_streaming CHECK (
    CASE
      WHEN original_streaming IS NULL THEN original_streaming_state IS NULL
      WHEN original_streaming IS 0 THEN original_streaming_state IS NULL
      ELSE 1
    END
    AND CASE
      WHEN original_streaming_state IS 1 THEN ghi_disabled IS 0
      WHEN original_streaming_state IS 2 THEN ghi_disabled IS 1
      ELSE 1
    END
  )
  , UNIQUE(file_id)
  , FOREIGN KEY (file_id, asset_type) REFERENCES AssetFile(file_id, asset_type)
) STRICT;

CREATE TABLE ImageFile (
  file_id INTEGER PRIMARY KEY NOT NULL
  , asset_type INTEGER NOT NULL DEFAULT 1
    CONSTRAINT const_asset_type CHECK (asset_type = 1)
  , image_format_name TEXT NOT NULL
  , UNIQUE(file_id)
  , FOREIGN KEY (file_id, asset_type) REFERENCES AssetFile(file_id, asset_type)
) STRICT;

CREATE TABLE DuplicateFile (
  dup_file_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , file_id INTEGER NOT NULL
  , root_dir_id INTEGER NOT NULL
  , file_path TEXT NOT NULL
  , FOREIGN KEY (file_id) REFERENCES AssetFile(file_id)
  , FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(asset_root_dir_id)
  , UNIQUE(root_dir_id, file_path)
) STRICT;

CREATE TABLE AssetThumbnail (
  thumbnail_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , file_id INTEGER NOT NULL
  -- 0 = large original aspect ratio, 1 = small cropped square
  , ty INTEGER NOT NULL
    CONSTRAINT bool_ty CHECK(ty IN (0, 1))
  , width INTEGER NOT NULL
    CONSTRAINT valid_width CHECK(width > 0)
  , height INTEGER NOT NULL
    CONSTRAINT valid_height CHECK(height > 0)
  , format_name TEXT NOT NULL
  , FOREIGN KEY (file_id) REFERENCES AssetFile(file_id)
  , CONSTRAINT unique_thumbnail UNIQUE(file_id, ty, width, height, format_name)
) STRICT;

CREATE TABLE VideoRepresentation (
  video_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , file_id INTEGER NOT NULL
  , name TEXT NOT NULL
    CONSTRAINT valid_name CHECK(NAME != '')
  , codec_name TEXT NOT NULL
  , width INTEGER
  , height INTEGER
  , bitrate INTEGER
  , created_status INTEGER NOT NULL
    CONSTRAINT enum_created_status CHECK(created_status IN (0, 1))
  , CONSTRAINT opt_cols_status CHECK(created_status = 0 OR (
      width IS NOT NULL
      AND height IS NOT NULL
      AND bitrate IS NOT NULL
  ))
  , FOREIGN KEY (file_id) REFERENCES VideoFile(file_id)
) STRICT;

CREATE TABLE AudioRepresentation (
  audio_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , file_id INTEGER NOT NULL
  , name TEXT NOT NULL
    CONSTRAINT valid_name CHECK(NAME != '')
  , codec_name TEXT NOT NULL
  , created_status INTEGER NOT NULL
    CONSTRAINT enum_created_status CHECK(created_status IN (0, 1))
  , FOREIGN KEY (file_id) REFERENCES VideoFile(file_id)
) STRICT;

CREATE TABLE ImageRepresentation (
  image_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  -- 0: created automatically
  -- ?: for example in-camera jpeg bundled in raw file
  , repr_type INTEGER NOT NULL DEFAULT 0
  , file_id INTEGER NOT NULL
  , format_name TEXT NOT NULL
  , width INTEGER NOT NULL
  , height INTEGER NOT NULL
  , file_size INTEGER NOT NULL
  , file_key TEXT NOT NULL
  , FOREIGN KEY (file_id) REFERENCES ImageFile(file_id)
) STRICT;

CREATE TABLE AlbumThumbnail (
  thumbnail_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , album_id INTEGER NOT NULL
  , format_name TEXT NOT NULL
  , width INTEGER NOT NULL
  , height INTEGER NOT NULL
  , file_key TEXT NOT NULL
  , FOREIGN KEY (album_id) REFERENCES Album(album_id)
  , UNIQUE (album_id, format_name, width, height)
  , UNIQUE (file_key)
) STRICT;

CREATE TABLE Album (
  album_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , name TEXT
  , description TEXT
  -- UTC timestamp in milliseconds since UNIX epoch
  , created_at INTEGER NOT NULL
  -- UTC timestamp in milliseconds since UNIX epoch
  , changed_at INTEGER NOT NULL
) STRICT;

-- -- surrogate key here because
-- -- https://dba.seriesexchange.com/a/761
CREATE TABLE AlbumItem (
  album_item_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , album_id INTEGER NOT NULL
  -- 1 = asset, 2 = text
  , ty INTEGER NOT NULL
  , asset_id INTEGER
  , text TEXT
  , idx INTEGER NOT NULL
  , UNIQUE(album_id, idx)
  , CONSTRAINT valid_type_data CHECK(
    (ty = 1 AND asset_id IS NOT NULL AND text IS NULL)
    OR
    (ty = 2 AND asset_id IS NULL AND text IS NOT NULL)
  )
  , FOREIGN KEY (album_id) REFERENCES Album(album_id)
  , FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
) STRICT;

CREATE INDEX album_id_index ON AlbumItem(album_id);

CREATE TABLE TimelineGroup (
  timeline_group_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , name TEXT
  -- UTC timestamp in milliseconds since UNIX epoch
  , created_at INTEGER NOT NULL
  -- UTC timestamp in milliseconds since UNIX epoch
  , changed_at INTEGER NOT NULL
) STRICT;

CREATE TABLE TimelineGroupItem (
  timeline_group_item_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL
  , group_id INTEGER NOT NULL
  , asset_id INTEGER NOT NULL
  -- an Asset can only belong to one TimelineGroup
  , UNIQUE(asset_id)
  , FOREIGN KEY (group_id) REFERENCES TimelineGroup(timeline_group_id)
  , FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
) STRICT;

CREATE TABLE GhiSegmentCache (
  ghi_cache_id INTEGER PRIMARY KEY NOT NULL
  , file_id INTEGER NOT NULL
  , file_name TEXT NOT NULL
  , size INTEGER
  -- 0: creation started, not finished
  -- 1: created
  , status INTEGER NOT NULL
    CONSTRAINT enum_status CHECK(status IN (0, 1))
  , created_at INTEGER NOT NULL DEFAULT (unixepoch())
  , accessed_at INTEGER NOT NULL DEFAULT (unixepoch())
  , UNIQUE(file_id, file_name)
  , CONSTRAINT valid_status_size CHECK((status = 0 AND size IS NULL) OR (status = 1 AND size IS NOT NULL))
  , CONSTRAINT opt_cols_size CHECK(size IS NULL OR size > 0)
  , FOREIGN KEY (file_id) REFERENCES VideoFile(file_id)
) STRICT;

CREATE TRIGGER GhiSegmentCache_Update_Accessed
AFTER UPDATE OF accessed_at
ON GhiSegmentCache
BEGIN
  UPDATE GhiSegmentCache SET accessed_at = (unixepoch())
  WHERE GhiSegmentCache.file_id = NEW.file_id
  AND GhiSegmentCache.file_name = NEW.file_name;
END;

-- =================== Configuration =======================

CREATE TABLE AcceptableVideoCodec (
  codec_name TEXT PRIMARY KEY NOT NULL
  , CONSTRAINT is_lowercase CHECK(LOWER(codec_name) = codec_name)
) STRICT;

CREATE TABLE AcceptableAudioCodec (
  codec_name TEXT PRIMARY KEY NOT NULL
  , CONSTRAINT is_lowercase CHECK(LOWER(codec_name) = codec_name)
) STRICT;

CREATE UNIQUE INDEX index_asset_repfile ON Asset(rep_file_id);
CREATE INDEX index_asset_date ON Asset(taken_date);
CREATE INDEX index_thumbnail_file ON AssetThumbnail(file_id);
CREATE INDEX index_imagerepr_file ON ImageRepresentation(file_id);
CREATE INDEX index_audiorepr_file ON AudioRepresentation(file_id);
CREATE INDEX index_videorepr_file ON VideoRepresentation(file_id);
CREATE INDEX index_asset_series ON Asset(series_id);
CREATE INDEX index_assetfile_asset ON AssetFile(asset_id);
CREATE INDEX index_timelinegroupitem_asset ON TimelineGroupItem(asset_id);
CREATE INDEX index_timelinegroupitem_group ON TimelineGroupItem(group_id);
CREATE INDEX index_imagerepresentation_file_id ON ImageRepresentation(file_id);
CREATE INDEX index_videorepresentation_file_id ON VideoRepresentation(file_id);
CREATE INDEX index_ghicache_file_id_name ON GhiSegmentCache(file_id, file_name);
