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
  is_auto INTEGER NOT NULL CHECK (is_auto IN (0, 1))
) STRICT;

CREATE TABLE Asset (
  asset_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
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
  thumb_hash BLOB,

  series_id INTEGER,
  is_series_selection INTEGER
  CHECK ((series_id IS NULL) = (is_series_selection IS NULL) AND is_series_selection IN (0, 1, NULL)),

  -- Metadata
  -- exiftool -j -g
  exiftool_output BLOB NOT NULL,
  -- latitude and longitude are stored multipled by 10e8
  gps_latitude INTEGER,
  gps_longitude INTEGER,

  -- 0: not a motion photo
  -- 1: motion photo with embedded video inside
  -- 2: this is the photo part of a motion photo split into 2 files/assets
  -- 3: this is the video part of a motion photo split into 2 files assets
  motion_photo INTEGER NOT NULL,
  -- asset_id of photo/video asset belonging to this motion photo
  motion_photo_assoc_asset_id INTEGER CHECK ((motion_photo = 2 OR motion_photo = 3) = (motion_photo_assoc_asset_id IS NOT NULL)),
  motion_photo_pts_us INTEGER CHECK ((motion_photo IS NULL) = (motion_photo_pts_us IS NOT NULL)),
  motion_photo_video_file_id INTEGER CHECK ((motion_photo = 1) = (motion_photo_video_file_id IS NOT NULL)),

  -- columns for images only
  image_format_name TEXT,

  -- columns for videos only
  ffprobe_output BLOB,
  video_codec_name TEXT,
  video_bitrate INTEGER,
  video_duration_ms INTEGER,
  audio_codec_name TEXT,
  has_dash INTEGER,

  FOREIGN KEY (series_id) REFERENCES AssetSeries(series_id),
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(asset_root_dir_id),
  UNIQUE(root_dir_id, file_path),
  FOREIGN KEY (motion_photo_assoc_asset_id) REFERENCES Asset(asset_id),
  FOREIGN KEY (motion_photo_video_file_id) REFERENCES MotionPhotoVideoFile(file_id),

  -- timezone_offset NULL is only valid for timezone_info=UtcCertain, and NoTimestamp I guess?
  CHECK (timezone_info IN (1, 2, 3, 4, 5, 6) AND (timezone_info IN (2, 6) OR timezone_offset IS NOT NULL)),

  CHECK(has_dash IN (0, 1)),
  -- valid Image or Video
  CHECK((ty = 1
      AND image_format_name IS NOT NULL
      AND ffprobe_output IS NULL
      AND video_codec_name IS NULL
      AND video_bitrate IS NULL
      AND video_duration_ms IS NULL
      AND audio_codec_name IS NULL
      AND has_dash IS NULL)
    OR (
      ty = 2 
      AND image_format_name IS NULL
      AND ffprobe_output IS NOT NULL
      AND video_codec_name IS NOT NULL
      AND video_bitrate IS NOT NULL 
      AND has_dash IS NOT NULL
      -- audio_codec_name, video_duration_ms can be null if there's no audio stream
  )),

  CHECK((gps_latitude IS NULL AND gps_longitude IS NULL) OR (gps_latitude IS NOT NULL AND gps_longitude IS NOT NULL))
) STRICT;

CREATE TABLE DuplicateAsset (
  dup_asset_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id),
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(asset_root_dir_id),
  UNIQUE(root_dir_id, file_path)
) STRICT;

CREATE TABLE AssetThumbnail (
  thumbnail_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  -- 0 = large original aspect ratio, 1 = small cropped square
  ty INTEGER NOT NULL CHECK(ty IN (0, 1)),
  width INTEGER NOT NULL CHECK(width > 0),
  height INTEGER NOT NULL CHECK(height > 0),
  format_name TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id),
  UNIQUE(asset_id, ty, width, height, format_name)
) STRICT;

CREATE TABLE VideoRepresentation (
  video_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  -- columns that aren't known until encoding is done can be null if is_preallocated_dummy is true
  codec_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  bitrate INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  media_info_key TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
) STRICT;

CREATE TABLE AudioRepresentation (
  audio_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  codec_name TEXT NOT NULL,
  -- bitrate INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  media_info_key TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
) STRICT;

CREATE TABLE ImageRepresentation (
  image_repr_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  format_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  file_size INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
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

CREATE TABLE MotionPhotoVideoFile (
  file_id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  FOREIGN KEY(asset_id) REFERENCES Asset(asset_id)
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
  -- UTC timestamp of date used to position the group in the timeline
  display_date INTEGER NOT NULL,
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

CREATE VIEW TimelineSegment (
timeline_id,
  asset_id,
  asset_taken_date,
  timeline_group_id,
  date_day,
  sort_date,
  series_sort_date,
  segment_idx
) AS 
WITH timeline AS (
  SELECT
  Asset.asset_id AS asset_id,
  Asset.taken_date AS asset_taken_date,
  CASE WHEN TimelineGroup.timeline_group_id IS NULL THEN date(Asset.taken_date / 1000, 'unixepoch') ELSE NULL END AS asset_date_day,
  CASE WHEN TimelineGroup.timeline_group_id IS NOT NULL THEN TimelineGroup.timeline_group_id ELSE NULL END AS group_id,
  CASE WHEN TimelineGroup.timeline_group_id IS NOT NULL THEN TimelineGroup.display_date ELSE Asset.taken_date END AS sort_date,
  CASE WHEN Asset.series_id IS NULL THEN Asset.taken_date ELSE 
      (SELECT MIN(a.taken_date) FROM Asset a WHERE a.series_id = Asset.series_id)
  END AS series_sort_date
  FROM Asset
  LEFT JOIN TimelineGroupItem ON TimelineGroupItem.asset_id = Asset.asset_id
  LEFT JOIN TimelineGroup ON TimelineGroupItem.group_id = TimelineGroup.timeline_group_id
  WHERE Asset.is_hidden = 0
)
-- assign segment numbers ignoring maximum segment size
SELECT 
0 as timeline_id,
asset_id,
asset_taken_date,
group_id AS timeline_group_id,
asset_date_day AS date_day,
sort_date,
series_sort_date,
DENSE_RANK() OVER 
(
  ORDER BY 
  date(sort_date / 1000, 'unixepoch') DESC, -- we store milliseconds, sqlite uses seconds
  CASE WHEN timeline.group_id IS NOT NULL THEN timeline.group_id ELSE 0 END,
  CASE WHEN timeline.group_id IS NOT NULL THEN 0 ELSE timeline.asset_date_day END
) AS segment_idx_no_max_size
FROM timeline
ORDER BY sort_date DESC, series_sort_date DESC, timeline_group_id DESC, asset_taken_date DESC, asset_id DESC;

-- =================== Configuration =======================

CREATE TABLE AcceptableVideoCodec (
  codec_name TEXT PRIMARY KEY NOT NULL,
  CHECK(LOWER(codec_name) = codec_name)
) STRICT;

CREATE TABLE AcceptableAudioCodec (
  codec_name TEXT PRIMARY KEY NOT NULL,
  CHECK(LOWER(codec_name) = codec_name)
) STRICT;

-- =================== Housekeeping ========================

CREATE TABLE FailedThumbnailJob (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  file_hash BLOB NOT NULL,
  -- milliseconds since UNIX epoch
  date INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
) STRICT;

CREATE TABLE FailedFFmpeg (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  file_hash BLOB NOT NULL,
  -- milliseconds since UNIX epoch
  date INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
);

CREATE TABLE FailedShakaPackager (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  file_hash BLOB NOT NULL,
  -- milliseconds since UNIX epoch
  date INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id)
);

CREATE TABLE DeletedAutoAssetSeries (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  series_id INTEGER NOT NULL, -- intentionally not a foreign key
  FOREIGN KEY (asset_id) REFERENCES Asset(asset_id) ON DELETE CASCADE
);
