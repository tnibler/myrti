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
  ty INTEGER NOT NULL CHECK(ty IN (1, 2)),
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL,
  file_type TEXT NOT NULL,
  hash BLOB,
  added_at TEXT NOT NULL,
  -- with zone offset if we know it, otherwise no offset and just assume local time
  taken_date TEXT,
  taken_date_local_fallback TEXT,
  -- width and height of the image/video as it is displayed, all metadata taken into account
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  -- rotation correction applied after exif/metadata rotation if that's still wrong
  rotation_correction INTEGER,
  thumb_small_square_avif TEXT,
  thumb_small_square_webp TEXT,
  thumb_large_orig_avif TEXT,
  thumb_large_orig_webp TEXT,
  thumb_small_square_width INTEGER,
  thumb_small_square_height INTEGER,
  thumb_large_orig_width INTEGER,
  thumb_large_orig_height INTEGER,

  -- columns for images only

  -- columns for videos only
  video_codec_name TEXT,
  video_bitrate INTEGER,
  audio_codec_name TEXT,
  has_dash INTEGER,

  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(id) ON DELETE CASCADE,
  UNIQUE(root_dir_id, file_path),

  CHECK(has_dash = 0 OR has_dash = 1),
  CHECK((ty = 1
          AND video_codec_name IS NULL
          AND video_bitrate IS NULL
          AND audio_codec_name IS NULL
          AND has_dash IS NULL)
        OR (
          ty = 2 
          AND video_codec_name IS NOT NULL
          AND video_bitrate IS NOT NULL 
          AND has_dash IS NOT NULL
          -- audio_codec_name can be null if there's no audio stream
  )),
  CHECK(taken_date IS NOT NULL OR taken_date_local_fallback IS NOT NULL)
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
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE AudioRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  codec_name TEXT NOT NULL,
  -- bitrate INTEGER NOT NULL,
  file_key TEXT NOT NULL,
  media_info_key TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE
) STRICT;

CREATE TABLE Album (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name TEXT,
  description TEXT,
  created_at TEXT NOT NULL,
  changed_at TEXT NOT NULL
) STRICT;

-- -- surrogate key here because
-- -- https://dba.stackexchange.com/a/761
CREATE TABLE AlbumEntry (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  album_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  UNIQUE(album_id, idx),
  FOREIGN KEY (album_id) REFERENCES Album(id) ON DELETE CASCADE,
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX album_id_index ON AlbumEntry(album_id);

CREATE TABLE FailedThumbnailJob (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  file_hash BLOB NOT NULL,
  date TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE
) STRICT;
