CREATE TABLE Country (
  geoname_id INTEGER PRIMARY KEY NOT NULL,
  iso TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  continent TEXT NOT NULL,
  neighbors TEXT NOT NULL
) STRICT;

CREATE TABLE CountryGeometry (
  geoname_id INTEGER PRIMARY KEY NOT NULL,
  bbox_xmin REAL NOT NULL,
  bbox_ymin REAL NOT NULL,
  bbox_xmax REAL NOT NULL,
  bbox_ymax REAL NOT NULL,
  geojson TEXT NOT NULL,
  FOREIGN KEY (geoname_id) REFERENCES Country(geoname_id)
) STRICT;

CREATE TABLE Geoname (
  geoname_id INTEGER PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  latitude REAL NOT NULL,
  longitude REAL NOT NULL,
  feature_class TEXT NOT NULL,
  feature_code TEXT NOT NULL,
  country_code TEXT NOT NULL,
  admin1_code TEXT NOT NULL,
  admin2_code TEXT NOT NULL,
  admin3_code TEXT NOT NULL,
  admin4_code TEXT NOT NULL,
  timezone TEXT NOT NULL,
  modification_date TEXT NOT NULL,
  
  FOREIGN KEY (country_code) REFERENCES Country(iso)
) STRICT;

-- when table Country was populated
CREATE TABLE CountryListPresent (
  id INTEGER PRIMARY KEY CHECK(id = 0),
  added_at INTEGER NOT NULL
) STRICT;

-- when table CountryGeometry was populated
CREATE TABLE CountryGeometryPresent (
  id INTEGER PRIMARY KEY CHECK(id = 0),
  added_at INTEGER NOT NULL
) STRICT;

-- when geoname info for Country was added to table Geoname
CREATE TABLE CountryDataPresent (
  geoname_id INTEGER PRIMARY KEY NOT NULL,
  -- Unix timestamp in seconds
  added_at INTEGER NOT NULL
) STRICT;

