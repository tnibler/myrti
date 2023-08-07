CREATE TABLE AssetRootDirs (
  id    INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path  TEXT NOT NULL UNIQUE
);

CREATE TABLE DataDirs (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path TEXT NOT NULL UNIQUE
);

CREATE TABLE ResourceFiles (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  data_dir_id INTEGER NOT NULL,
  path_in_data_dir TEXT NOT NULL,
  created_at DATETIME NOT NULL,
  UNIQUE(data_dir_id, path_in_data_dir),
  FOREIGN KEY (data_dir_id) REFERENCES DataDirs(id) ON DELETE CASCADE
);

CREATE TABLE Assets (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  ty INTEGER NOT NULL CHECK(ty IN (1, 2)),
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL UNIQUE,
  hash BLOB,
  added_at DATETIME NOT NULL,
  file_created_at DATETIME,
  file_modified_at DATETIME,
  canonical_date DATETIME,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  thumb_small_square_jpg INTEGER,
  thumb_small_square_webp INTEGER,
  thumb_large_orig_jpg INTEGER,
  thumb_large_orig_webp INTEGER,
  thumb_small_square_width INTEGER,
  thumb_small_square_height INTEGER,
  thumb_large_orig_width INTEGER,
  thumb_large_orig_height INTEGER,
  FOREIGN KEY (thumb_small_square_jpg) REFERENCES ResourceFiles(id) ON DELETE SET NULL,
  FOREIGN KEY (thumb_small_square_webp) REFERENCES ResourceFiles(id) ON DELETE SET NULL,
  FOREIGN KEY (thumb_large_orig_jpg) REFERENCES ResourceFiles(id) ON DELETE SET NULL,
  FOREIGN KEY (thumb_large_orig_webp) REFERENCES ResourceFiles(id) ON DELETE SET NULL,
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDirs(id) ON DELETE CASCADE
);

CREATE TABLE ImageInfo (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Assets(id) ON DELETE CASCADE
);

CREATE TABLE VideoInfo (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  dash_resource_dir INTEGER,
  FOREIGN KEY (asset_id) REFERENCES Assets(id) ON DELETE CASCADE,
  FOREIGN KEY (dash_resource_dir) REFERENCES ResourceFiles(id) ON DELETE SET NULL
);

CREATE TABLE VideoRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  codec_name TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  bitrate INTEGER NOT NULL,
  resource_file INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Assets(id) ON DELETE CASCADE,
  FOREIGN KEY (resource_file) REFERENCES ResourceFiles(id) ON DELETE CASCADE
);

CREATE TABLE AudioRepresentation (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  asset_id INTEGER NOT NULL,
  codec_name TEXT NOT NULL,
  bitrate INTEGER NOT NULL,
  resource_file INTEGER NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Assets(id) ON DELETE CASCADE,
  FOREIGN KEY (resource_file) REFERENCES ResourceFiles(id) ON DELETE CASCADE
);

CREATE TABLE Albums (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name TEXT,
  description TEXT,
  created_at DATETIME NOT NULL,
  changed_at DATETIME NOT NULL
);

-- -- surrogate key here because
-- -- https://dba.stackexchange.com/a/761
CREATE TABLE AlbumEntries (
  id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  album_id INTEGER NOT NULL,
  asset_id INTEGER NOT NULL,
  idx INTEGER NOT NULL,
  UNIQUE(album_id, idx),
  FOREIGN KEY (album_id) REFERENCES Albums(id) ON DELETE CASCADE
);

CREATE INDEX album_id_index ON AlbumEntries(album_id);
