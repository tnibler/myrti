use std::{cell::RefCell, collections::HashSet, str::FromStr};

use camino::Utf8Path as Path;
use chrono::{DateTime, Utc};
use color_eyre::{
    eyre::{eyre, Context},
    Report, Result,
};
use futures::StreamExt;
use geo::{Contains, Coord, Geometry};
use geojson::GeoJson;
use serde::Deserialize;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use thiserror::Error;
use tracing::info;

mod download;
pub mod geoname_schema;
mod ingest;

type DbPool = sqlx::Pool<Sqlite>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct GeonameId(pub i64);

pub struct ReverseGeocoder {
    pool: DbPool,
    index: usearch::Index,
    countries_in_index: RefCell<HashSet<GeonameId>>,
}

#[derive(Debug, Copy, Clone)]
pub struct Coordinates {
    pub lat: f32,
    pub lon: f32,
}

#[derive(Error, Debug)]
pub enum ReverseGeocodeError {
    #[error("basic data not present in database")]
    BaseDataNotPresent,
    #[error("country data not present in database")]
    CountryDataNotPresent { country_id: GeonameId },
    #[error("other error")]
    Other(#[from] Report),
}

#[derive(Debug, Clone)]
pub struct LookupResult {
    pub geoname_id: GeonameId,
    pub name: String,
    pub coords: Coordinates,
    pub feature_class: String,
    pub feature_type: String,
    pub country_code: String,
    pub country_id: GeonameId,
}

impl ReverseGeocoder {
    pub async fn new(db_path: &Path) -> Result<ReverseGeocoder> {
        let db_url = format!("sqlite://{}", db_path);
        if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            info!("Creating database {}", db_url);
            Sqlite::create_database(&db_url).await?;
        }

        let pool = SqlitePool::connect(&db_url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        let index_options = usearch::ffi::IndexOptions {
            multi: false,
            dimensions: 2,
            metric: usearch::ffi::MetricKind::Haversine,
            quantization: usearch::ffi::ScalarKind::F16,
            connectivity: 0,
            expansion_add: 0,
            expansion_search: 0,
        };
        let index = usearch::new_index(&index_options).unwrap();
        index.reserve(100000).unwrap();
        Ok(ReverseGeocoder {
            pool,
            index,
            countries_in_index: Default::default(),
        })
    }

    pub async fn close(self) {
        self.pool.close().await;
    }

    pub async fn lookup(
        &self,
        coord: Coordinates,
    ) -> Result<Option<LookupResult>, ReverseGeocodeError> {
        let base_data_exists = country_list_exists(&self.pool)
            .await
            .map_err(|e| ReverseGeocodeError::Other(e))?;
        if let None = base_data_exists {
            return Err(ReverseGeocodeError::BaseDataNotPresent);
        }
        // first find all countries for which the point is in the bounding box(es)
        let bbox_candidates = country_candidates_by_bbox(coord, &self.pool).await?;
        let mut country: Option<GeonameId> = None;
        // then check wich country geometry coord is actually inside of
        for country_id in bbox_candidates {
            let contains = country_contains(country_id, coord, &self.pool)
                .await
                .unwrap();
            if contains {
                debug_assert!(country.is_none());
                country = Some(country_id);
                #[cfg(not(debug_assertions))]
                break;
            }
        }
        let country_id = if let Some(country) = country {
            country
        } else {
            return Ok(None);
        };

        let country_data_exists = country_data_exists(country_id, &self.pool).await?;
        if let None = country_data_exists {
            return Err(ReverseGeocodeError::CountryDataNotPresent { country_id });
        }
        if !self.countries_in_index.borrow().contains(&country_id) {
            add_country_to_index(country_id, &self.pool, &self.index)
                .await
                .wrap_err("error adding country data to index")?;
            self.countries_in_index.borrow_mut().insert(country_id);
        }
        let result_geoname_id = self
            .index
            .search(&[coord.lat, coord.lon], 1)
            .wrap_err("error searching usearch index")?
            .keys[0] as i64;
        let geoname_row = sqlx::query!(
            r#"
SELECT * FROM Geoname 
WHERE geoname_id = ?;
        "#,
            result_geoname_id
        )
        .fetch_one(&self.pool)
        .await
        .wrap_err("error getting result row from table Geoname")?;
        Ok(Some(LookupResult {
            geoname_id: GeonameId(result_geoname_id),
            name: geoname_row.name,
            coords: Coordinates {
                lat: geoname_row.latitude as f32,
                lon: geoname_row.longitude as f32,
            },
            feature_class: geoname_row.feature_class,
            feature_type: geoname_row.feature_code,
            country_code: geoname_row.country_code,
            country_id,
        }))
    }

    pub async fn base_data_present(&self) -> Result<Option<DateTime<Utc>>> {
        let country_list_date = match country_list_exists(&self.pool).await? {
            None => return Ok(None),
            Some(d) => d,
        };
        let country_geometry_date = match country_geometry_exists(&self.pool).await? {
            None => return Ok(None),
            Some(d) => d,
        };
        Ok(Some(std::cmp::min(
            country_geometry_date,
            country_list_date,
        )))
    }

    pub async fn download_base_data(&self) -> Result<()> {
        let country_list_read = download::download_file_reader("countryInfo.txt")
            .await
            .wrap_err("could not download country list (countryInfo.txt)")?;
        ingest::ingest_country_info(country_list_read, &self.pool)
            .await
            .wrap_err("error ingesting country list (countryInfo.txt)")?;

        let country_shapes_path = tempfile::Builder::new()
            .tempfile()
            .wrap_err("error creating temp file")?
            .into_temp_path();
        download::download_zipped_file(
            "shapes_all_low.zip",
            "shapes_all_low.txt",
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&country_shapes_path)
                .wrap_err("error creating output file for shapes_all_low.txt")?,
        )
        .await
        .wrap_err("could not download country geometry (shapes_all_low.txt)")?;
        ingest::ingest_country_geojson(
            tokio::fs::File::open(&country_shapes_path)
                .await
                .wrap_err("could not open downloaded file")?,
            &self.pool,
        )
        .await
        .wrap_err("error ingesting country geometry (shapes_all_low.txt)")?;
        Ok(())
    }

    pub async fn download_country_data(&self, country_id: GeonameId) -> Result<()> {
        let country_code = sqlx::query!(
            r#"
SELECT iso FROM Country
WHERE geoname_id = ?;
        "#,
            country_id.0
        )
        .fetch_one(&self.pool)
        .await
        .wrap_err("could not get row from table Country")?
        .iso;
        let country_data_path = tempfile::Builder::new()
            .tempfile()
            .wrap_err("error creating temp file")?
            .into_temp_path();
        download::download_zipped_file(
            &format!("{}.zip", country_code.to_ascii_uppercase()),
            &format!("{}.txt", country_code.to_ascii_uppercase()),
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&country_data_path)
                .wrap_err("error creating output file for country data")?,
        )
        .await
        .wrap_err("could not download country geometry (shapes_all_low.txt)")?;
        ingest::ingest_geoname_data(
            tokio::fs::File::open(&country_data_path)
                .await
                .wrap_err("could not open downloaded file")?,
            &self.pool,
        )
        .await
        .wrap_err("error ingesting country data")?;
        let now = chrono::Utc::now().timestamp();
        sqlx::query!(
            r#"
INSERT INTO CountryDataPresent VALUES (?, ?);
    "#,
            country_id.0,
            now
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

async fn add_country_to_index(
    country_id: GeonameId,
    pool: &DbPool,
    index: &usearch::Index,
) -> Result<()> {
    let country_code = sqlx::query!(
        r#"
SELECT iso FROM COUNTRY
WHERE geoname_id = ?;
    "#,
        country_id.0
    )
    .fetch_one(pool)
    .await
    .wrap_err("could not query table Country")?
    .iso;
    let mut rows = sqlx::query!(
        r#"
SELECT geoname_id, latitude, longitude FROM Geoname 
WHERE country_code = ? AND feature_class = 'P'
AND feature_code IN ('PPL', 'PPLL', 'PPLS', 'PPLA', 'PPLA2', 'PPLA3', 'PPLA4', 'PPLA5', 'PPLC', 'PPLG');
    "#,
        country_code
    )
    .fetch_many(pool);
    while let Some(row) = rows.next().await {
        let row = row.wrap_err("error querying table Geoname")?;
        match row {
            sqlx::Either::Right(row) => {
                if index.size() == index.capacity() {
                    index
                        .reserve(index.size() + 10000)
                        .wrap_err("error growing index capacity")?;
                }
                index
                    .add(
                        row.geoname_id as u64,
                        &[row.latitude as f32, row.longitude as f32],
                    )
                    .wrap_err("error adding entry to index")?;
            }
            sqlx::Either::Left(_) => {}
        }
    }
    Ok(())
}

async fn country_contains(
    geoname_id: GeonameId,
    coord: Coordinates,
    pool: &DbPool,
) -> Result<bool> {
    let geojson_str = sqlx::query!(
        r#"
SELECT geojson FROM CountryGeometry WHERE geoname_id = ?;
    "#,
        geoname_id.0
    )
    .fetch_one(pool)
    .await?
    .geojson;
    let geojson = GeoJson::from_str(&geojson_str).unwrap();
    let geom: geo::Geometry<f32> = geojson.try_into()?;
    match &geom {
        Geometry::Polygon(polygon) => Ok(polygon.contains(&Coord {
            x: coord.lon,
            y: coord.lat,
        })),
        Geometry::MultiPolygon(multipolygon) => Ok(multipolygon.contains(&Coord {
            x: coord.lon,
            y: coord.lat,
        })),
        _ => return Err(eyre!("bad geometry type")),
    }
}

async fn country_candidates_by_bbox(coord: Coordinates, pool: &DbPool) -> Result<Vec<GeonameId>> {
    #[derive(Deserialize)]
    struct QueryResult {
        pub geoname_id: i64,
    }
    // geojson is lon, lat!
    let results: Vec<_> = sqlx::query_as!(
        QueryResult,
        r#"
SELECT Country.geoname_id as geoname_id FROM Country, CountryGeometry
WHERE bbox_xmin < $1 AND bbox_xmax > $1
AND bbox_ymin < $2 AND bbox_ymax > $2
AND Country.geoname_id = CountryGeometry.geoname_id;
    "#,
        coord.lon,
        coord.lat,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| GeonameId(r.geoname_id))
    .collect();
    Ok(results)
}

async fn country_list_exists(pool: &DbPool) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let result = sqlx::query!(
        r#"
SELECT added_at
FROM CountryListPresent;
    "#,
    )
    .fetch_optional(pool)
    .await
    .wrap_err("could not query table CountryListPresent")?;
    let added_at = result
        .map(|r| {
            chrono::DateTime::from_timestamp(r.added_at, 0)
                .ok_or(eyre!("invalid timestamp in column added_at"))
        })
        .transpose()?;
    Ok(added_at)
}

async fn country_geometry_exists(pool: &DbPool) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let result = sqlx::query!(
        r#"
SELECT added_at
FROM CountryGeometryPresent;
    "#,
    )
    .fetch_optional(pool)
    .await
    .wrap_err("could not query table CargoGeometryPresent")?;
    let added_at = result
        .map(|r| {
            chrono::DateTime::from_timestamp(r.added_at, 0)
                .ok_or(eyre!("invalid timestamp in column added_at"))
        })
        .transpose()?;
    Ok(added_at)
}

pub async fn country_data_exists(
    country_geoname_id: GeonameId,
    pool: &DbPool,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let result = sqlx::query!(
        r#"
SELECT geoname_id, added_at
FROM CountryDataPresent
WHERE geoname_id = ?;
    "#,
        country_geoname_id.0
    )
    .fetch_optional(pool)
    .await
    .wrap_err("could not query table CountryDataPresent")?;
    let added_at = result
        .map(|r| {
            chrono::DateTime::from_timestamp(r.added_at, 0)
                .ok_or(eyre!("invalid timestamp in column added_at"))
        })
        .transpose()?;
    Ok(added_at)
}
