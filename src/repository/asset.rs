use color_eyre::eyre;
use eyre::{Context, Result};
use sqlx::{Executor, Sqlite, SqliteConnection, SqliteExecutor, Transaction};
use std::path::Path;
use tracing::instrument;

use crate::model::{
    db_entity::{DbAsset, DbVideoInfo},
    AssetAll, AssetBase, AssetId, AssetType, FullAsset, Image, Video,
};

use super::pool::DbPool;

#[instrument(name = "Insert FullAsset", skip(pool, asset), level = "debug")]
pub async fn insert_asset(pool: &DbPool, asset: FullAsset) -> Result<AssetId> {
    debug_assert!(
        asset.base.ty
            == match asset.asset {
                AssetAll::Image(_) => AssetType::Image,
                AssetAll::Video(_) => AssetType::Video,
            }
    );
    let mut tx = pool.begin().await?;
    let id = insert_asset_base(&mut tx, &asset.base).await?;
    match &asset.asset {
        AssetAll::Image(image) => {
            insert_image_info(&mut tx, id, image).await?;
        }
        AssetAll::Video(video) => {
            insert_video_info(&mut tx, id, video).await?;
        }
    };
    tx.commit().await?;
    Ok(id)
}

#[instrument(name = "Update FullAsset", skip(pool, asset), fields(id=%asset.base.id), level = "debug")]
pub async fn update_asset(pool: &DbPool, asset: FullAsset) -> Result<()> {
    debug_assert!(
        asset.base.ty
            == match asset.asset {
                AssetAll::Image(_) => AssetType::Image,
                AssetAll::Video(_) => AssetType::Video,
            }
    );
    let mut tx = pool.begin().await?;
    update_asset_base(&mut tx, &asset.base).await?;
    let id = asset.base.id;
    match &asset.asset {
        AssetAll::Image(image) => {
            update_image_info(&mut tx, id, image).await?;
        }
        AssetAll::Video(video) => {
            update_video_info(&mut tx, id, video).await?;
        }
    };
    tx.commit().await?;
    Ok(())
}

#[instrument(name = "Get AssetBase with path", skip(pool), level = "debug")]
pub async fn get_asset_with_path(pool: &DbPool, path: &Path) -> Result<Option<AssetBase>> {
    let path = path.to_str().unwrap();
    let db_asset = sqlx::query_as!(
        DbAsset,
        r#"
SELECT id,
ty as "ty: _",
root_dir_id,
file_path,
hash,
added_at,
file_created_at,
file_modified_at,
canonical_date,
thumb_path_small_square_jpg,
thumb_path_small_square_webp,
thumb_path_large_orig_jpg,
thumb_path_large_orig_webp
FROM Assets WHERE file_path = ?;
    "#,
        path
    )
    .fetch_optional(pool)
    .await?;
    db_asset.map(|db_asset| db_asset.try_into()).transpose()
}

#[instrument(name = "Get all AssetBases", skip(pool), level = "debug")]
pub async fn get_assets(pool: &DbPool) -> Result<Vec<AssetBase>> {
    sqlx::query_as!(
        DbAsset,
        r#"
SELECT id,
ty as "ty: _",
root_dir_id,
file_path,
hash,
added_at,
file_created_at,
file_modified_at,
canonical_date,
thumb_path_small_square_jpg,
thumb_path_small_square_webp,
thumb_path_large_orig_jpg,
thumb_path_large_orig_webp
FROM Assets;
    "#
    )
    // TODO don't collect into vec before mapping
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r: DbAsset| AssetBase::try_from(r))
    .collect::<Result<Vec<AssetBase>>>()
}

#[instrument(
    name = "Get AssetBases with missing thumbnails",
    skip(pool),
    level = "debug"
)]
pub async fn get_assets_with_missing_thumbnail(
    pool: &DbPool,
    limit: Option<i32>,
) -> Result<Vec<AssetBase>> {
    if let Some(limit) = limit {
        sqlx::query_as!(
            DbAsset,
            r#"
SELECT id,
ty as "ty: _",
root_dir_id,
file_path,
hash,
added_at,
file_created_at,
file_modified_at,
canonical_date,
thumb_path_small_square_jpg,
thumb_path_small_square_webp,
thumb_path_large_orig_jpg,
thumb_path_large_orig_webp
FROM Assets
WHERE   
    thumb_path_small_square_jpg IS NULL OR
    thumb_path_small_square_webp IS NULL OR
    thumb_path_large_orig_jpg IS NULL OR
    thumb_path_large_orig_jpg IS NULL
LIMIT ?;
    "#,
            limit
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| r.try_into())
        .collect()
    } else {
        sqlx::query_as!(
            DbAsset,
            r#"
SELECT id,
ty as "ty: _",
root_dir_id,
file_path,
hash,
added_at,
file_created_at,
file_modified_at,
canonical_date,
thumb_path_small_square_jpg,
thumb_path_small_square_webp,
thumb_path_large_orig_jpg,
thumb_path_large_orig_webp
FROM Assets
WHERE   
    thumb_path_small_square_jpg IS NULL OR
    thumb_path_small_square_webp IS NULL OR
    thumb_path_large_orig_jpg IS NULL OR
    thumb_path_large_orig_jpg IS NULL;
    "#
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| r.try_into())
        .collect()
    }
}

#[instrument(name = "Update AssetBase", skip(conn, asset), fields(id=%asset.id), level = "debug")]
pub async fn update_asset_base(conn: &mut SqliteConnection, asset: &AssetBase) -> Result<()> {
    debug_assert!(asset.id.0 != 0);
    let db_asset_base: DbAsset = asset.try_into()?;
    sqlx::query!(
        "
UPDATE Assets SET 
ty=?,
root_dir_id=?,
file_path=?,
hash=?,
added_at=?,
file_created_at=?,
file_modified_at=?,
canonical_date=?,
thumb_path_small_square_jpg=?,
thumb_path_small_square_webp=?,
thumb_path_large_orig_jpg=?,
thumb_path_large_orig_webp=?
WHERE id=?;
",
        db_asset_base.ty,
        db_asset_base.root_dir_id.0,
        db_asset_base.file_path,
        db_asset_base.hash,
        db_asset_base.added_at,
        db_asset_base.file_created_at,
        db_asset_base.file_modified_at,
        db_asset_base.canonical_date,
        db_asset_base.thumb_path_small_square_jpg,
        db_asset_base.thumb_path_small_square_webp,
        db_asset_base.thumb_path_large_orig_jpg,
        db_asset_base.thumb_path_large_orig_webp,
        asset.id.0
    )
    .execute(conn)
    .await
    .wrap_err("could not update table Assets")?;
    Ok(())
}

#[instrument(name = "Insert AssetBase", skip(conn, asset), level = "debug")]
pub async fn insert_asset_base(conn: &mut SqliteConnection, asset: &AssetBase) -> Result<AssetId> {
    debug_assert!(asset.id.0 == 0);
    let db_asset_base: DbAsset = asset.try_into()?;
    let result = sqlx::query!(
        "
INSERT INTO Assets
(id,
ty,
root_dir_id,
file_path,
hash,
added_at,
file_created_at,
file_modified_at,
canonical_date,
thumb_path_small_square_jpg,
thumb_path_small_square_webp,
thumb_path_large_orig_jpg,
thumb_path_large_orig_webp)
VALUES
(null, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
",
        db_asset_base.ty,
        db_asset_base.root_dir_id.0,
        db_asset_base.file_path,
        db_asset_base.hash,
        db_asset_base.added_at,
        db_asset_base.file_created_at,
        db_asset_base.file_modified_at,
        db_asset_base.canonical_date,
        db_asset_base.thumb_path_small_square_jpg,
        db_asset_base.thumb_path_small_square_webp,
        db_asset_base.thumb_path_large_orig_jpg,
        db_asset_base.thumb_path_large_orig_webp,
    )
    .execute(conn)
    .await
    .wrap_err("could not insert into table Assets")?;
    let rowid = result.last_insert_rowid();
    Ok(AssetId(rowid))
}

#[instrument(name = "Insert ImageInfo", skip(conn, image), level = "debug")]
pub async fn insert_image_info(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    image: &Image,
) -> Result<()> {
    debug_assert!(asset_id.0 != 0);
    let db_image_info = image.try_to_db_image_info(asset_id)?;
    sqlx::query!("INSERT INTO ImageInfo (asset_id) VALUES(?);", asset_id.0,)
        .execute(conn)
        .await
        .wrap_err("could not insert into table ImageInfo")?;
    Ok(())
}

#[instrument(name = "Update ImageInfo", skip(conn, image), level = "debug")]
pub async fn update_image_info(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    image: &Image,
) -> Result<()> {
    debug_assert!(asset_id.0 != 0);
    let db_image_info = image.try_to_db_image_info(asset_id)?;
    Ok(())
}

#[instrument(name = "Insert VideoInfo", skip(conn, video), level = "debug")]
pub async fn insert_video_info(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    video: &Video,
) -> Result<()> {
    debug_assert!(asset_id.0 != 0);
    let db_video_info: DbVideoInfo = video.try_to_db_video_info(asset_id)?;
    sqlx::query!(
        "
INSERT INTO VideoInfo (asset_id, dash_manifest_path) VALUES
(?, ?);
",
        asset_id.0,
        db_video_info.dash_manifest_path
    )
    .execute(conn)
    .await
    .wrap_err("could not insert into table VideoInfo")?;
    Ok(())
}

#[instrument(name = "Update VideoInfo", skip(conn, video), level = "debug")]
pub async fn update_video_info(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    video: &Video,
) -> Result<()> {
    debug_assert!(asset_id.0 != 0);
    let db_video_info: DbVideoInfo = video.try_to_db_video_info(asset_id)?;
    sqlx::query!(
        "
UPDATE VideoInfo SET dash_manifest_path=? 
WHERE asset_id=?;
",
        db_video_info.dash_manifest_path,
        asset_id.0
    )
    .execute(conn)
    .await
    .wrap_err("could not update table VideoInfo")?;
    Ok(())
}
