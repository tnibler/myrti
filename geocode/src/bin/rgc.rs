use std::collections::HashSet;
use std::eprintln;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use color_eyre::eyre::{eyre, Context, Result};
use futures::{StreamExt, TryStreamExt};
use geo::BoundingRect;
use geo::{algorithm::contains::Contains, Coord, Geometry};
use geocode::download::{download_file_reader, download_zipped_file};
use geocode::{country_data_exists, country_geometry_exists, country_list_exists, download};
use geojson::GeoJson;
use serde::Deserialize;
use sqlx::QueryBuilder;
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};

type DbPool = sqlx::Pool<Sqlite>;

use geocode::geoname_schema::{CountryInfoCsv, GeoJsonCsv, GeonameCsv};

pub async fn create_db() -> Result<DbPool> {
    let db_url = "sqlite://geodata.db";
    if !Sqlite::database_exists(db_url).await.unwrap_or(false) {
        Sqlite::create_database(db_url).await?;
    }
    // } else {
    //     println!("dropping and recreating database");
    //     Sqlite::drop_database(db_url).await?;
    //     Sqlite::create_database(db_url).await?;
    // }

    let pool = SqlitePool::connect(db_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn ingest_country_info(
    read: impl tokio::io::AsyncRead + Unpin + Send,
    pool: &DbPool,
) -> Result<()> {
    sqlx::query!(
        r#"
DELETE FROM Country;
        "#
    )
    .execute(pool)
    .await
    .wrap_err("could not delete from table Country")?;
    sqlx::query!(
        r#"
DELETE FROM CountryListPresent;
        "#
    )
    .execute(pool)
    .await
    .wrap_err("could not delete from table CountryListPresent")?;
    let mut records = csv_async::AsyncReaderBuilder::new()
        .has_headers(false)
        .comment(Some(b'#'))
        .delimiter(b'\t')
        .create_reader(read)
        .into_records();
    let no_longer_exist: HashSet<i64> = [
        5880801, // American Samoa
        8505032, // Netherlands Antilles
        8505033, // Serbia and Montenegro
    ]
    .into();
    while let Some(record) = records.next().await {
        let country_info: CountryInfoCsv = record?.deserialize(None)?;
        if no_longer_exist.contains(&country_info.geoname_id) {
            continue;
        }
        sqlx::query!(
            r#"
INSERT INTO Country(
geoname_id,
iso,
name,
continent,
neighbors
) VALUES (?, ?, ?, ?, ?);
        "#,
            country_info.geoname_id,
            country_info.iso,
            country_info.country,
            country_info.continent,
            country_info.neighbors
        )
        .execute(pool)
        .await?;
    }
    let now = chrono::Utc::now().timestamp();
    sqlx::query!(
        r#"
INSERT INTO CountryListPresent VALUES (0, ?);
    "#,
        now
    )
    .execute(pool)
    .await
    .wrap_err("could not insert into table CountryListPresent")?;
    Ok(())
}

pub async fn ingest_country_geojson(
    read: impl tokio::io::AsyncRead + Unpin + Send,
    pool: &DbPool,
) -> Result<()> {
    let mut records = csv_async::AsyncReaderBuilder::new()
        .has_headers(true)
        .comment(Some(b'#'))
        .delimiter(b'\t')
        .create_reader(read)
        .into_records();
    while let Some(record) = records.next().await {
        dbg!(&record);
        let record: GeoJsonCsv = record?.deserialize(None)?;
        let exists = sqlx::query!(
            r#"
SELECT * FROM Country WHERE geoname_id=?;
        "#,
            record.geoname_id
        )
        .fetch_optional(pool)
        .await?
        .is_some();
        if !exists {
            continue;
        }
        let geojson = GeoJson::from_str(&record.geojson)?;
        let geometry: geo::Geometry<f32> = geojson.try_into()?;
        let bbox = match geometry {
            Geometry::Polygon(polygon) => polygon.bounding_rect().expect("why can this fail"),
            Geometry::MultiPolygon(polygon) => polygon.bounding_rect().expect("why can this fail"),
            _ => {
                return Err(eyre!("unexpected geometry type"));
            }
        };
        let (bbox_x0, bbox_y0) = bbox.min().x_y();
        let (bbox_x1, bbox_y1) = bbox.max().x_y();
        sqlx::query!(
            r#"
INSERT INTO CountryGeometry(
geoname_id, 
bbox_xmin,
bbox_ymin,
bbox_xmax,
bbox_ymax,
geojson
) VALUES (?, ?, ?, ?, ?, ?);
        "#,
            record.geoname_id,
            bbox_x0,
            bbox_y0,
            bbox_x1,
            bbox_y1,
            record.geojson,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn ingest_geoname_data(path: &Path, pool: &DbPool) -> Result<()> {
    let file = tokio::fs::File::open(path).await?;
    let mut records = csv_async::AsyncReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .create_reader(file)
        .into_records()
        .chunks(1000);
    while let Some(chunk) = records.next().await {
        let geoname_csvs = chunk
            .into_iter()
            .map(|record| record.unwrap().deserialize::<GeonameCsv>(None).unwrap());
        let mut qb = QueryBuilder::new(
            r#"
INSERT INTO Geoname(
geoname_id,
name,
latitude,
longitude,
feature_class,
feature_code,
country_code,
admin1_code,
admin2_code,
admin3_code,
admin4_code,
timezone,
modification_date
)
        "#,
        );
        qb.push_values(geoname_csvs, move |mut builder, s| {
            builder.push_bind(s.geoname_id);
            builder.push_bind(s.name);
            builder.push_bind(s.latitude);
            builder.push_bind(s.longitude);
            builder.push_bind(s.feature_class);
            builder.push_bind(s.feature_code);
            builder.push_bind(s.country_code);
            builder.push_bind(s.admin1_code);
            builder.push_bind(s.admin2_code);
            builder.push_bind(s.admin3_code);
            builder.push_bind(s.admin4_code);
            builder.push_bind(s.timezone);
            builder.push_bind(s.modification_date);
        });
        qb.push(";");
        qb.build().execute(pool).await?;
    }
    Ok(())
}

pub async fn country_bbox_candidates(lat: f32, lon: f32, pool: &DbPool) -> Result<Vec<i64>> {
    #[derive(Deserialize)]
    struct QueryResult {
        pub geoname_id: i64,
    }
    // geojson is lon, lat!
    let results: Vec<i64> = sqlx::query_as!(
        QueryResult,
        r#"
SELECT Country.geoname_id as geoname_id FROM Country, CountryGeometry
WHERE bbox_xmin < ? AND bbox_xmax > ?
AND bbox_ymin < ? AND bbox_ymax > ?
AND Country.geoname_id = CountryGeometry.geoname_id;
    "#,
        lon,
        lon,
        lat,
        lat
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| r.geoname_id)
    .collect();
    Ok(results)
}

pub async fn country_contains(geoname_id: i64, lat: f32, lon: f32, pool: &DbPool) -> Result<bool> {
    let geojson_str = sqlx::query!(
        r#"
SELECT geojson FROM CountryGeometry WHERE geoname_id = ?;
    "#,
        geoname_id
    )
    .fetch_one(pool)
    .await?
    .geojson;
    let geojson = GeoJson::from_str(&geojson_str).unwrap();
    let geom: geo::Geometry<f32> = geojson.try_into()?;
    match &geom {
        Geometry::Polygon(polygon) => Ok(polygon.contains(&Coord { x: lon, y: lat })),
        Geometry::MultiPolygon(multipolygon) => {
            Ok(multipolygon.contains(&Coord { x: lon, y: lat }))
        }
        _ => return Err(eyre!("bad geometry type")),
    }
}

pub async fn build_usearch_index(pool: &DbPool, country_code: &str) -> usearch::Index {
    let options = usearch::ffi::IndexOptions {
        multi: false,
        dimensions: 2,
        metric: usearch::ffi::MetricKind::Haversine,
        quantization: usearch::ffi::ScalarKind::F16,
        connectivity: 0,
        expansion_add: 0,
        expansion_search: 0,
    };
    let index = usearch::new_index(&options).unwrap();
    index.reserve(100000).unwrap();
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
        match row.unwrap() {
            sqlx::Either::Right(row) => {
                index
                    .add(
                        row.geoname_id as u64,
                        &[row.latitude as f32, row.longitude as f32],
                    )
                    .unwrap();
            }
            sqlx::Either::Left(_) => {}
        }
    }
    return index;
}

fn parse_lat_lon(s: &str) -> Result<(f32, f32)> {
    match s.trim().split(",").map(|s| s.trim()).collect::<Vec<_>>() {
        split if split.len() == 2 => {
            let f: Vec<f32> = split
                .into_iter()
                .map(|s| s.parse().wrap_err("parse error"))
                .collect::<Result<_>>()?;
            Ok((f[0], f[1]))
        }
        _ => Err(eyre!("invalid input")),
    }
}

#[tokio::main]
async fn main() {
    println!("initializing");
    let pool = create_db().await.unwrap();

    if let None = country_list_exists(&pool).await.unwrap() {
        eprintln!("downloading countryList.txt");
        let country_list_read = download_file_reader("countryInfo.txt").await.unwrap();
        ingest_country_info(country_list_read, &pool).await.unwrap();
    }

    if let None = country_geometry_exists(&pool).await.unwrap() {
        eprintln!("downloading shapes_all_low.txt");
        let country_shapes_path = tempfile::Builder::new()
            .tempfile()
            .unwrap()
            .into_temp_path();
        download_zipped_file(
            "shapes_all_low.zip",
            "shapes_all_low.txt",
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open("shapes_all_low.txt")
                .unwrap(),
        )
        .await
        .unwrap();
        ingest_country_geojson(
            tokio::fs::File::open(&country_shapes_path).await.unwrap(),
            &pool,
        )
        .await
        .unwrap();
    }
    // ingest_geoname_data(&PathBuf::from("./data/DE.txt"), &pool)
    //     .await
    //     .unwrap();

    let index = build_usearch_index(&pool, "DE").await;
    println!("ready");
    let mut input = String::new();
    while let Ok(_) = std::io::stdin().read_line(&mut input) {
        let (lat, lon): (f32, f32) = match parse_lat_lon(&input) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("error parsing input");
                input.clear();
                continue;
            }
        };
        //         let result_ids = index.search(&[lat, lon], 10).unwrap();
        //         for (id, distance) in result_ids
        //             .keys
        //             .into_iter()
        //             .zip(result_ids.distances.into_iter())
        //         {
        //             let id = id as i64;
        //             let row = sqlx::query!(
        //                 r#"
        // SELECT * FROM Geoname WHERE geoname_id = ?;
        //             "#,
        //                 id
        //             )
        //             .fetch_one(&pool)
        //             .await
        //             .unwrap();
        //             println!("{}: {}", row.name, distance);
        //         }
        let bbox_candidates = country_bbox_candidates(lat, lon, &pool).await.unwrap();
        for country_id in bbox_candidates {
            // if let None = country_data_exists(country_id, &pool).await.unwrap() {
            //     println!("country data not present");
            // } else {
            //     println!("country present")
            // }
            // let contains = country_contains(country_id, lat, lon, &pool).await.unwrap();
            // if contains {
            //     let name =
            //         sqlx::query!("SELECT name FROM Country WHERE geoname_id = ?;", country_id)
            //             .fetch_one(&pool)
            //             .await
            //             .unwrap()
            //             .name;
            //     println!("{}, {}: {}", lat, lon, name);
            // }
        }
        input.clear();
    }

    pool.close().await;
}
