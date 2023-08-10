CREATE TABLE AssetRootDir (
  id    INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path  TEXT NOT NULL UNIQUE
);

CREATE TABLE DataDir (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path TEXT NOT NULL UNIQUE
);

CREATE TABLE ResourceFile (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  data_dir_id INTEGER NOT NULL,
  path_in_data_dir TEXT NOT NULL,
  created_at DATETIME NOT NULL,
  UNIQUE(data_dir_id, path_in_data_dir),
  FOREIGN KEY (data_dir_id) REFERENCES DataDir(id) ON DELETE CASCADE
);

CREATE TABLE Asset (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  ty INTEGER NOT NULL CHECK(ty IN (1, 2)),
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL UNIQUE,
  hash BLOB,
  added_at DATETIME NOT NULL,
  -- with zone offset if we know it, otherwise no offset and just assume local time
  taken_date DATETIME,
  taken_date_local_fallback DATETIME,
  -- width and height of the image/video as it is displayed, all metadata taken into account
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  -- rotation correction applied after exif/metadata rotation if that's still wrong
  -- rotation_correction INTEGER,
  thumb_small_square_jpg INTEGER,
  thumb_small_square_webp INTEGER,
  thumb_large_orig_jpg INTEGER,
  thumb_large_orig_webp INTEGER,
  thumb_small_square_width INTEGER,
  thumb_small_square_height INTEGER,
  thumb_large_orig_width INTEGER,
  thumb_large_orig_height INTEGER,
  FOREIGN KEY (thumb_small_square_jpg) REFERENCES ResourceFile(id) ON DELETE SET NULL,
  FOREIGN KEY (thumb_small_square_webp) REFERENCES ResourceFile(id) ON DELETE SET NULL,
  FOREIGN KEY (thumb_large_orig_jpg) REFERENCES ResourceFile(id) ON DELETE SET NULL,
  FOREIGN KEY (thumb_large_orig_webp) REFERENCES ResourceFile(id) ON DELETE SET NULL,
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDir(id) ON DELETE CASCADE,
  CHECK(taken_date IS NOT NULL OR taken_date_local_fallback IS NOT NULL)
);

CREATE TABLE ImageInfo (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE
);

CREATE TABLE VideoInfo (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  codec_name TEXT NOT NULL,
  dash_resource_dir INTEGER,
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE,
  FOREIGN KEY (dash_resource_dir) REFERENCES ResourceFile(id) ON DELETE SET NULL
);

CREATE TABLE VideoRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  codec_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  bitrate INTEGER NOT NULL,
  path_in_resource_dir TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE
);

CREATE TABLE AudioRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  -- codec_name TEXT NOT NULL,
  -- bitrate INTEGER NOT NULL,
  path_in_resource_dir TEXT NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Asset(id) ON DELETE CASCADE
);

CREATE TABLE Album (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name TEXT,
  description TEXT,
  created_at DATETIME NOT NULL,
  changed_at DATETIME NOT NULL
);

-- -- surrogate key here because
-- -- https://dba.stackexchange.com/a/761
CREATE TABLE AlbumEntry (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  album_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  UNIQUE(album_id, idx),
  FOREIGN KEY (album_id) REFERENCES Album(id) ON DELETE CASCADE
);

CREATE INDEX album_id_index ON AlbumEntry(album_id);
