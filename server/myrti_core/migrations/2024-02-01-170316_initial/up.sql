CREATE TABLE AssetRootDir (
  asset_root_dir_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE DataDir (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE AssetSeries (
  series_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  -- 0 unknown
  -- 1 burst shot
  -- 2 timelapse
  series_type INTEGER NOT NULL,
  is_auto INTEGER NOT NULL CHECK (is_auto IN (0, 1))
) STRICT;

CREATE TABLE Asset (
  asset_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  -- 1=Image, 2=Video
  asset_type INTEGER NOT NULL CHECK (asset_type IN (1, 2)),
  rep_file_id INTEGER NOT NULL UNIQUE,
  is_hidden INTEGER NOT NULL CHECK (is_hidden IN (0, 1)),

  -- UTC timestamp in milliseconds since UNIX epoch
  taken_date INTEGER NOT NULL,
  -- "+03:00"
  timezone_offset TEXT,
  timezone_info INTEGER NOT NULL,

  series_id INTEGER,
  is_series_selection INTEGER
  CHECK ((series_id IS NULL) = (is_series_selection IS NULL) AND is_series_selection IN (0, 1, NULL)),

  -- latitude and longitude are stored multipled by 10e8
  gps_latitude INTEGER,
  gps_longitude INTEGER,

  UNIQUE(asset_id, asset_type),

  -- timezone_offset NULL is only valid for timezone_info=UtcCertain, and NoTimestamp I guess?
  CHECK (timezone_info IN (1, 2, 3, 4, 5, 6) AND (timezone_info IN (2, 6) OR timezone_offset IS NOT NULL)),
  FOREIGN KEY (rep_file_id, asset_type) REFERENCES AssetFile(file_id, asset_type),
  UNIQUE(rep_file_id),
  UNIQUE(rep_file_id, asset_type),
  FOREIGN KEY (series_id) REFERENCES AssetSeries(series_id),
  CHECK((gps_latitude IS NULL AND gps_longitude IS NULL) OR (gps_latitude IS NOT NULL AND gps_longitude IS NOT NULL))
);

CREATE TABLE AssetFile (
  file_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  -- 1=Image, 2=Video
  asset_type INTEGER NOT NULL CHECK (asset_type IN (1, 2)),
  root_dir_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  merge_reason INTEGER DEFAULT NULL,
  file_path TEXT NOT NULL,
  file_type TEXT NOT NULL, -- FileType from exiftool
  hash BLOB UNIQUE,
  -- UTC timestamp in milliseconds since UNIX epoch
  added_at INTEGER NOT NULL,
  -- width and height of the image/video as it is displayed, all metadata taken into account
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  -- rotation correction applied after exif/metadata rotation if that's still wrong
  rotation_correction INTEGER NOT NULL DEFAULT 0 CHECK(rotation_correction IN (0, 1, 2, 3)),
  -- 0: Nothing, 1: Horizontal, 2: Vertical, 3: Both
  mirror_correction INTEGER NOT NULL DEFAULT 0 CHECK(mirror_correction IN (0, 1, 2, 3)),
  thumb_hash BLOB,

  -- Metadata
  -- exiftool -j -g
  exiftool_output BLOB NOT NULL,
  file_name TEXT GENERATED ALWAYS AS (json_extract(exiftool_output, '$[0].File.FileName')) VIRTUAL,
  dir_name TEXT GENERATED ALWAYS AS (json_extract(exiftool_output, '$[0].File.Directory')) VIRTUAL,
  file_ext TEXT GENERATED ALWAYS AS (json_extract(exiftool_output, '$[0].File.FileTypeExtension')) VIRTUAL,
  file_stem TEXT GENERATED ALWAYS AS (
      CASE
         WHEN file_name LIKE '%.' || file_ext
         THEN substr(file_name, 1, length(file_name) - length(file_ext) - 1)
         ELSE file_name
       END
    ) STORED,

  FOREIGN KEY (asset_id, asset_type) REFERENCES Asset(asset_id, asset_type) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(asset_root_dir_id),
  UNIQUE(root_dir_id, file_path),
  UNIQUE(hash),
  -- FKs in Video/ImageAsset need to reference a candidate key in Asset.
  -- Any set of columns (asset_id, ...) obviously fulfills that, but sqlite doesn't know so it needs an explicit index
  UNIQUE(file_id, asset_id),
  UNIQUE(file_id, asset_type)
) STRICT;

CREATE TABLE VideoFile (
  file_id INTEGER PRIMARY KEY NOT NULL,
  asset_type INTEGER NOT NULL CHECK (asset_type = 2) DEFAULT 2,
  ffprobe_output BLOB NOT NULL,
  video_codec_name TEXT NOT NULL,
  video_bitrate INTEGER,
  video_duration_ms INTEGER,
  audio_codec_name TEXT,
  frame_rate_num INTEGER CHECK(frame_rate_num IS NULL OR frame_rate_num > 0),
  frame_rate_denom INTEGER CHECK(frame_rate_denom IS NULL OR frame_rate_denom > 0),

  is_original_streamable INTEGER CHECK(is_original_streamable IN (NULL, 0, 1)),
  -- NULL: unknown
  -- 0: none
  -- 1: video
  -- 2: audio
  -- 3: video+audio
  has_ghi INTEGER CHECK(has_ghi IN (0, 1, 2, 3)),
  max_iframe_interval INTEGER,
  CHECK((frame_rate_num IS NULL) = (frame_rate_denom IS NULL)),
  CHECK(is_original_streamable IN (0, NULL) OR
    (max_iframe_interval IS NOT NULL
      AND frame_rate_num IS NOT NULL
      AND frame_rate_denom IS NOT NULL
  )),

  UNIQUE(file_id),
  FOREIGN KEY (file_id, asset_type) REFERENCES AssetFile(file_id, asset_type)
) STRICT;

CREATE TABLE ImageFile (
  file_id INTEGER PRIMARY KEY NOT NULL,
  asset_type INTEGER NOT NULL CHECK (asset_type = 1) DEFAULT 1,
  image_format_name TEXT NOT NULL,
  UNIQUE(file_id),
  FOREIGN KEY (file_id, asset_type) REFERENCES AssetFile(file_id, asset_type)
) STRICT;

CREATE TABLE DuplicateFile (
  dup_file_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  file_id INTEGER NOT NULL,
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL,
  FOREIGN KEY (file_id) REFERENCES AssetFile(file_id),
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(asset_root_dir_id),
  UNIQUE(root_dir_id, file_path)
) STRICT;

CREATE TABLE AssetThumbnail (
  thumbnail_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  file_id INTEGER NOT NULL,
  -- 0 = large original aspect ratio, 1 = small cropped square
  ty INTEGER NOT NULL CHECK(ty IN (0, 1)),
  width INTEGER NOT NULL CHECK(width > 0),
  height INTEGER NOT NULL CHECK(height > 0),
  format_name TEXT NOT NULL,
  FOREIGN KEY (file_id) REFERENCES AssetFile(file_id),
  UNIQUE(file_id, ty, width, height, format_name)
) STRICT;

CREATE TABLE VideoRepresentation (
  video_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  file_id INTEGER NOT NULL,
  name TEXT NOT NULL CHECK(NAME != ''),
  codec_name TEXT NOT NULL,
  width INTEGER,
  height INTEGER,
  bitrate INTEGER,
  created_status INTEGER NOT NULL CHECK(created_status IN (0, 1)),
  CHECK(created_status = 0 OR (
      width IS NOT NULL
      AND height IS NOT NULL
      AND bitrate IS NOT NULL
  )),
  FOREIGN KEY (file_id) REFERENCES VideoFile(file_id)
) STRICT;

CREATE TABLE AudioRepresentation (
  audio_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  file_id INTEGER NOT NULL,
  name TEXT NOT NULL CHECK(NAME != ''),
  codec_name TEXT NOT NULL,
  created_status INTEGER NOT NULL CHECK(created_status IN (0, 1)),
  FOREIGN KEY (file_id) REFERENCES VideoFile(file_id)
) STRICT;

CREATE TABLE ImageRepresentation (
  image_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  -- 0: created automatically
  -- ?: for example in-camera jpeg bundled in raw file
  repr_type INTEGER NOT NULL DEFAULT 0,
  file_id INTEGER NOT NULL,
  format_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  file_size INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  FOREIGN KEY (file_id) REFERENCES ImageFile(file_id)
) STRICT;

CREATE TABLE AlbumThumbnail (
  thumbnail_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  album_id INTEGER NOT NULL,
  format_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  FOREIGN KEY (album_id) REFERENCES Album(album_id),
  UNIQUE (album_id, format_name, width, height),
  UNIQUE (file_key)
) STRICT;

CREATE TABLE Album (
  album_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name TEXT,
  description TEXT,
  -- UTC timestamp in milliseconds since UNIX epoch
  created_at INTEGER NOT NULL,
  -- UTC timestamp in milliseconds since UNIX epoch
  changed_at INTEGER NOT NULL
) STRICT;

-- -- surrogate key here because
-- -- https://dba.seriesexchange.com/a/761
CREATE TABLE AlbumItem (
  album_item_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  album_id INTEGER NOT NULL,
  -- 1 = asset, 2 = text
  ty INTEGER NOT NULL,
  asset_id INTEGER,
  text TEXT,
  idx INTEGER NOT NULL,
  UNIQUE(album_id, idx),
  CHECK(
    (ty = 1 AND asset_id IS NOT NULL AND text IS NULL)
    OR
    (ty = 2 AND asset_id IS NULL AND text IS NOT NULL)
  ),
  FOREIGN KEY (album_id) REFERENCES Album(album_id),
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
) STRICT;

CREATE INDEX album_id_index ON AlbumItem(album_id);

CREATE TABLE TimelineGroup (
  timeline_group_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name TEXT,
  -- UTC timestamp in milliseconds since UNIX epoch
  created_at INTEGER NOT NULL,
  -- UTC timestamp in milliseconds since UNIX epoch
  changed_at INTEGER NOT NULL
) STRICT;

CREATE TABLE TimelineGroupItem (
  timeline_group_item_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  group_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  -- an Asset can only belong to one TimelineGroup
  UNIQUE(asset_id),
  FOREIGN KEY (group_id) REFERENCES TimelineGroup(timeline_group_id),
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
) STRICT;

CREATE TABLE GhiSegmentCache (
  video_repr_id INTEGER
  , audio_repr_id INTEGER
  , file_name TEXT NOT NULL
  , size INTEGER NOT NULL
  , created_at INTEGER NOT NULL DEFAULT (unixepoch())
  , accessed_at INTEGER NOT NULL DEFAULT (unixepoch())
  , UNIQUE(video_repr_id, file_name)
  , UNIQUE(audio_repr_id, file_name)
  , CHECK ((video_repr_id IS NULL) IS NOT (audio_repr_id IS NULL))
  , FOREIGN KEY (video_repr_id) REFERENCES VideoRepresentation(video_repr_id)
  , FOREIGN KEY (audio_repr_id) REFERENCES AudioRepresentation(audio_repr_id)
) STRICT;

CREATE TRIGGER GhiSegmentCache_Update_Accessed
AFTER UPDATE OF accessed_at
ON GhiSegmentCache
BEGIN
  UPDATE GhiSegmentCache SET accessed_at = (unixepoch())
  WHERE GhiSegmentCache.video_repr_id = NEW.video_repr_id
  AND GhiSegmentCache.audio_repr_id = NEW.audio_repr_id;
END;

-- =================== Configuration =======================

CREATE TABLE AcceptableVideoCodec (
  codec_name TEXT PRIMARY KEY NOT NULL,
  CHECK(LOWER(codec_name) = codec_name)
) STRICT;

CREATE TABLE AcceptableAudioCodec (
  codec_name TEXT PRIMARY KEY NOT NULL,
  CHECK(LOWER(codec_name) = codec_name)
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
