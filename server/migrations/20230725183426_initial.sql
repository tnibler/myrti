PRAGMA foreign_keys = ON;

CREATE TABLE AssetRootDir (
  id    INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path  TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE DataDir (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path TEXT NOT NULL UNIQUE
) STRICT;

CREATE TABLE Asset (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  -- 1=Image, 2=Video
  ty INTEGER NOT NULL CHECK (ty IN (1, 2)),
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL,
  file_type TEXT NOT NULL,
  hash BLOB UNIQUE,
  is_hidden INTEGER NOT NULL CHECK (is_hidden IN (0, 1)),
  -- UTC timestamp in milliseconds since UNIX epoch
  added_at INTEGER NOT NULL,
  -- UTC timestamp in milliseconds since UNIX epoch
  taken_date INTEGER NOT NULL,
  -- "+03:00"
  timezone_offset TEXT,
  timezone_info INTEGER NOT NULL,
  -- width and height of the image/video as it is displayed, all metadata taken into account
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  -- rotation correction applied after exif/metadata rotation if that's still wrong
  rotation_correction INTEGER,

  -- Metadata
  -- latitude and longitude are stored multipled by 10e8
  gps_latitude INTEGER,
  gps_longitude INTEGER,

  thumb_small_square_avif INTEGER NOT NULL,
  thumb_small_square_webp INTEGER NOT NULL,
  thumb_large_orig_avif INTEGER NOT NULL,
  thumb_large_orig_webp INTEGER NOT NULL,
  thumb_small_square_width INTEGER,
  thumb_small_square_height INTEGER,
  thumb_large_orig_width INTEGER,
  thumb_large_orig_height INTEGER,

  -- columns for images only
  image_format_name TEXT,

  -- columns for videos only
  video_codec_name TEXT,
  video_bitrate INTEGER,
  audio_codec_name TEXT,
  has_dash INTEGER,

  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(id),
  UNIQUE(root_dir_id, file_path),

  -- timezone_offset NULL is only valid for timezone_info=UtcCertain, and NoTimestamp I guess?
  CHECK (timezone_info IN (1, 2, 3, 4, 5, 6) AND (timezone_info IN (2, 6) OR timezone_offset IS NOT NULL)),

  CHECK(has_dash IN (0, 1)),
  -- valid Image or Video
  CHECK((ty = 1
      AND image_format_name IS NOT NULL
      AND video_codec_name IS NULL
      AND video_bitrate IS NULL
      AND audio_codec_name IS NULL
      AND has_dash IS NULL)
    OR (
      ty = 2 
      AND image_format_name IS NULL
      AND video_codec_name IS NOT NULL
      AND video_bitrate IS NOT NULL 
      AND has_dash IS NOT NULL
      -- audio_codec_name can be null if there's no audio stream
  )),

  CHECK((gps_latitude IS NULL AND gps_longitude IS NULL) OR (gps_latitude IS NOT NULL AND gps_longitude IS NOT NULL))
) STRICT;

CREATE TABLE DuplicateAsset (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id),
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(id),
  UNIQUE(root_dir_id, file_path)
) STRICT;

CREATE TABLE VideoRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  codec_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  bitrate INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  media_info_key TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id)
) STRICT;

CREATE TABLE AudioRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  codec_name TEXT NOT NULL,
  -- bitrate INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  media_info_key TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id)
) STRICT;

CREATE TABLE ImageRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  format_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  file_size INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id),
  UNIQUE (asset_id, format_name, width, height)
) STRICT;

CREATE TABLE Album (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name TEXT,
  description TEXT,
  is_timeline_group INTEGER NOT NULL CHECK(is_timeline_group IN (0, 1)),
  -- UTC timestamp of date used to position the group in the timeline
  timeline_group_display_date INTEGER CHECK (is_timeline_group = 0 OR timeline_group_display_date IS NOT NULL),
  -- UTC timestamp in milliseconds since UNIX epoch
  created_at INTEGER NOT NULL,
  -- UTC timestamp in milliseconds since UNIX epoch
  changed_at INTEGER NOT NULL
) STRICT;

-- -- surrogate key here because
-- -- https://dba.stackexchange.com/a/761
CREATE TABLE AlbumEntry (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  album_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  UNIQUE(album_id, idx),
  FOREIGN KEY (album_id) REFERENCES Album(id),
  FOREIGN KEY (asset_id) REFERENCES Asset(id)
) STRICT;

CREATE INDEX album_id_index ON AlbumEntry(album_id);

CREATE TABLE FailedThumbnailJob (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  file_hash BLOB NOT NULL,
  -- milliseconds since UNIX epoch
  date INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id)
) STRICT;

CREATE TABLE FailedFFmpeg (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  file_hash BLOB NOT NULL,
  -- milliseconds since UNIX epoch
  date INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id)
);

CREATE TABLE FailedShakaPackager (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  file_hash BLOB NOT NULL,
  -- milliseconds since UNIX epoch
  date INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id)
);
