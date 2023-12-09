use std::{collections::HashSet, str::FromStr};

use color_eyre::{eyre::eyre, Result};
use futures::StreamExt;
use geo::{BoundingRect, Geometry};
use geojson::GeoJson;
use sqlx::QueryBuilder;
use tokio::io::AsyncRead;

use crate::{
    geoname_schema::{CountryInfoCsv, GeoJsonCsv, GeonameCsv},
    DbPool,
};

pub async fn ingest_country_info(read: impl AsyncRead + Unpin + Send, pool: &DbPool) -> Result<()> {
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
    .await?;
    Ok(())
}

pub async fn ingest_country_geojson(
    read: impl AsyncRead + Unpin + Send,
    pool: &DbPool,
) -> Result<()> {
    let mut records = csv_async::AsyncReaderBuilder::new()
        .has_headers(true)
        .comment(Some(b'#'))
        .delimiter(b'\t')
        .create_reader(read)
        .into_records();
    while let Some(record) = records.next().await {
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
    let now = chrono::Utc::now().timestamp();
    sqlx::query!(
        r#"
INSERT INTO CountryGeometryPresent VALUES (0, ?);
    "#,
        now
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn ingest_geoname_data(read: impl AsyncRead + Unpin + Send, pool: &DbPool) -> Result<()> {
    let mut records = csv_async::AsyncReaderBuilder::new()
        .has_headers(false)
        .delimiter(b'\t')
        .create_reader(read)
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
