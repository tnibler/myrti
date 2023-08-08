use super::pool::DbPool;
use crate::model::{repository::db_entity::DbDataDir, DataDir, DataDirId};
use eyre::{Context, Result};
use tracing::{instrument, Instrument};

#[instrument(name = "Get random DataDir", skip(pool))]
pub async fn get_random_data_dir(pool: &DbPool) -> Result<DataDir> {
    sqlx::query_as!(
        DbDataDir,
        r#"
SELECT id, path FROM DataDirs ORDER BY RANDOM() LIMIT 1;
    "#
    )
    .fetch_one(pool)
    .await
    .map(|d| d.try_into())?
    .wrap_err("could not query table DataDirs for random row")
}

#[instrument(name = "Insert DataDir", skip(pool))]
pub async fn insert_data_dir(pool: &DbPool, data_dir: DataDir) -> Result<DataDirId> {
    debug_assert!(data_dir.id.0 == 0);
    let data_dir: DbDataDir = data_dir
        .try_into()
        .wrap_err("could not insert into table DataDirs: path is not valid unicode")?;
    let result = sqlx::query!(
        r#"
INSERT INTO DataDirs
VALUES (NULL, ?);
    "#,
        data_dir.path
    )
    .execute(pool)
    .in_current_span()
    .await
    .wrap_err("could not insert into table DataDirs")?;
    let rowid = result.last_insert_rowid();
    Ok(DataDirId(rowid))
}

pub async fn get_data_dir_with_path(pool: &DbPool, path: &str) -> Result<Option<DataDir>> {
    sqlx::query_as!(
        DbDataDir,
        r#"
SELECT id, path FROM DataDirs WHERE path=?;
    "#,
        path
    )
    .fetch_optional(pool)
    .await
    .map(|o| o.map(|d| d.try_into()).transpose())?
    .wrap_err("could not query table DataDirs")
}
