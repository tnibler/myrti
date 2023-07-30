CREATE TABLE IF NOT EXISTS AssetRootDirs (
  id    INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  path  TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS Assets (
  id    INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  ty  INTEGER NOT NULL CHECK(ty IN (1, 2)),
  root_dir_id INTEGER NOT NULL,
  file_path TEXT NOT NULL UNIQUE,
  hash BLOB,
  added_at DATETIME NOT NULL,
  file_created_at DATETIME,
  file_modified_at DATETIME,
  canonical_date DATETIME,
  thumb_path_small_square_jpg TEXT,
  thumb_path_small_square_webp TEXT,
  thumb_path_large_orig_jpg TEXT,
  thumb_path_large_orig_webp TEXT,
  FOREIGN KEY (root_dir_id) REFERENCES AssetRootDirs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ImageInfo (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  FOREIGN KEY (asset_id) REFERENCES Assets(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS VideoInfo (
  asset_id INTEGER PRIMARY KEY NOT NULL,
  dash_manifest_path TEXT,
  FOREIGN KEY (asset_id) REFERENCES Assets(id) ON DELETE CASCADE
);
