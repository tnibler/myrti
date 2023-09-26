use camino::Utf8Path as Path;
use eyre::{Context, Result};
use tracing::Instrument;

use crate::model::{util::path_to_string, AssetId, AssetRootDirId, DuplicateAssetId};

use super::pool::DbPool;

#[derive(Debug, Clone)]
pub struct NewDuplicateAsset<'a> {
    pub existing_asset_id: AssetId,
    pub asset_root_dir_id: AssetRootDirId,
    pub path_in_asset_root: &'a Path,
}

#[tracing::instrument(skip(pool), level = "debug")]
pub async fn insert_duplicate_asset<'a>(
    pool: &DbPool,
    dupe: NewDuplicateAsset<'a>,
) -> Result<DuplicateAssetId> {
    let path = path_to_string(dupe.path_in_asset_root)?;
    let result = sqlx::query!(
        r#"
INSERT INTO DuplicateAsset VALUES (NULL, ?, ?, ?);
    "#,
        dupe.existing_asset_id,
        dupe.asset_root_dir_id,
        path
    )
    .execute(pool)
    .in_current_span()
    .await
    .wrap_err("could not insert into table DuplicateAsset")?;
    let id = DuplicateAssetId(result.last_insert_rowid());
    Ok(id)
}
