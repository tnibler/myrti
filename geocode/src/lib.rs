use color_eyre::{
    eyre::{eyre, Context},
    Result,
};
use sqlx::Sqlite;

pub mod download;
pub mod geoname_schema;
pub mod ingest;

pub type DbPool = sqlx::Pool<Sqlite>;

pub async fn country_list_exists(pool: &DbPool) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
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

pub async fn country_geometry_exists(
    pool: &DbPool,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
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
    country_geoname_id: i64,
    pool: &DbPool,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let result = sqlx::query!(
        r#"
SELECT geoname_id, added_at
FROM CountryDataPresent
WHERE geoname_id = ?;
    "#,
        country_geoname_id
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
