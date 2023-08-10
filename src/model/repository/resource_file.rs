use chrono::Utc;
use eyre::{Context, Result};
use sqlx::SqliteConnection;
use tracing::{debug, instrument, Instrument};

use crate::{
    core::NewResourceFile,
    model::{
        repository::db_entity::DbResourceFileResolved, util::path_to_string, ResourceFileId,
        ResourceFileResolved,
    },
};

use super::pool::DbPool;

#[instrument(name = "Insert new resource file", skip(conn))]
pub async fn insert_new_resource_file(
    conn: &mut SqliteConnection,
    new_resource_file: NewResourceFile,
) -> Result<ResourceFileId> {
    debug!("insert");
    let created_at = Utc::now().naive_utc();
    let path = path_to_string(new_resource_file.path_in_data_dir)
        .wrap_err("failed to insert new ResourceFile, couldn't convert path to String")?;
    let result = sqlx::query!(
        r#"
INSERT INTO ResourceFile
VALUES (NULL, ?, ?, ?); 
"#,
        new_resource_file.data_dir_id,
        path,
        created_at
    )
    .execute(conn)
    .in_current_span()
    .await
    .wrap_err("could not insert into table ResourceFiles")?;
    let rowid = result.last_insert_rowid();
    Ok(ResourceFileId(rowid))
}

pub async fn get_resource_file_resolved(
    pool: &DbPool,
    id: ResourceFileId,
) -> Result<ResourceFileResolved> {
    let result: ResourceFileResolved = sqlx::query_as!(
        DbResourceFileResolved,
        r#"
SELECT
ResourceFile.id,
data_dir_id,
path_in_data_dir,
DataDir.path as data_dir_path,
ResourceFile.created_at
FROM ResourceFile INNER JOIN DataDir 
ON ResourceFile.data_dir_id=DataDir.id 
WHERE ResourceFile.id=?;
    "#,
        id
    )
    .fetch_one(pool)
    .await
    .wrap_err("could not get ResourceFileResolved from db")?
    .try_into()?;
    Ok(result)
}
