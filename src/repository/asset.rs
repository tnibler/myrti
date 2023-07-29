use color_eyre::eyre;
use eyre::{Context, Result};
use sqlx::{Executor, Sqlite, SqliteConnection, SqliteExecutor, Transaction};
use std::path::Path;

use crate::model::{
    db_entity::{DbAsset, DbVideoInfo},
    AssetAll, AssetBase, AssetId, AssetType, FullAsset, Image, Video,
};

use super::pool::DbPool;

pub async fn insert_asset(pool: &DbPool, asset: FullAsset) -> Result<AssetId> {
    insert_or_update_asset(pool, asset, false).await
}

pub async fn update_asset(pool: &DbPool, asset: FullAsset) -> Result<AssetId> {
    insert_or_update_asset(pool, asset, true).await
}

async fn insert_or_update_asset(pool: &DbPool, asset: FullAsset, update: bool) -> Result<AssetId> {
    debug_assert!(
        asset.base.ty
            == match asset.asset {
                AssetAll::Image(_) => AssetType::Image,
                AssetAll::Video(_) => AssetType::Video,
            }
    );
    let mut tx = pool.begin().await?;
    let id: AssetId = if update {
        update_asset_base(&mut tx, &asset.base).await?;
        asset.base.id
    } else {
        insert_asset_base(&mut tx, &asset.base).await?
    };
    match &asset.asset {
        AssetAll::Image(image) => {
            if update {
                update_image_info(&mut tx, id, image).await?;
            } else {
                insert_image_info(&mut tx, id, image).await?;
            }
        }
        AssetAll::Video(video) => {
            if update {
                update_video_info(&mut tx, id, video).await?;
            } else {
                insert_video_info(&mut tx, id, video).await?;
            }
        }
    };
    tx.commit().await?;
    if update {
        debug_assert!(id == asset.base.id);
    }
    Ok(id)
}

pub async fn get_asset_with_path(pool: &DbPool, path: &Path) -> Result<Option<AssetBase>> {
    let path = path.to_str().unwrap();
    let db_asset = sqlx::query_as!(
            DbAsset,
            r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets WHERE file_path = ?;
    "#,
            path
        )
        .fetch_optional(pool)
        .await?;
    db_asset.map(|db_asset| db_asset.try_into()).transpose()
}

pub async fn get_assets(pool: &DbPool) -> Result<Vec<AssetBase>> {
    sqlx::query_as!(
        DbAsset,
        r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets;
    "#)
        // TODO don't collect into vec before mapping
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r: DbAsset| AssetBase::try_from(r))
        .collect::<Result<Vec<AssetBase>>>()
}

pub async fn get_assets_with_missing_thumbnail(
    pool: &DbPool,
    limit: Option<i32>,
) -> Result<Vec<AssetBase>> {
    if let Some(limit) = limit {
        sqlx::query_as!(DbAsset,
            r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets
WHERE thumb_path_jpg IS NULL OR thumb_path_webp IS NULL
LIMIT ?;
    "#, limit)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| r.try_into()).collect()
    } else {
        sqlx::query_as!(DbAsset,
            r#"
SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets
WHERE thumb_path_jpg IS NULL OR thumb_path_webp IS NULL;
    "#)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| r.try_into()).collect()
    }
}

pub async fn update_asset_base(conn: &mut SqliteConnection, asset: &AssetBase) -> Result<()> {
    debug_assert!(asset.id.0 != 0);
    let db_asset_base: DbAsset = asset.try_into()?;
    sqlx::query!(
"
UPDATE Assets SET ty=?, root_dir_id=?, file_path=?, file_created_at=?, file_modified_at=?, thumb_path_jpg=?, thumb_path_webp=? 
WHERE id=?;
",
            db_asset_base.ty,
            db_asset_base.root_dir_id.0,
            db_asset_base.file_path,
            db_asset_base.file_created_at,
            db_asset_base.file_modified_at,
            db_asset_base.thumb_path_jpg,
            db_asset_base.thumb_path_webp,
                asset.id.0
        ).execute(conn).await.wrap_err("could not update table Assets")?;
    Ok(())
}

pub async fn insert_asset_base(conn: &mut SqliteConnection, asset: &AssetBase) -> Result<AssetId> {
    debug_assert!(asset.id.0 == 0);
    let db_asset_base: DbAsset = asset.try_into()?;
    let result = sqlx::query!(
        "
INSERT INTO Assets (id, ty, root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp) VALUES
(null, ?, ?, ?, ?, ?, ?, ?);
",
        db_asset_base.ty,
        db_asset_base.root_dir_id,
        db_asset_base.file_path,
        db_asset_base.file_created_at,
        db_asset_base.file_modified_at,
        db_asset_base.thumb_path_jpg,
        db_asset_base.thumb_path_webp,
    ).execute(conn).await.wrap_err("could not insert into table Assets")?;
    let rowid = result.last_insert_rowid();
    Ok(AssetId(rowid))
}

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

pub async fn update_image_info(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    image: &Image,
) -> Result<()> {
    debug_assert!(asset_id.0 != 0);
    let db_image_info = image.try_to_db_image_info(asset_id)?;
    Ok(())
}

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

// pub async fn get_asset(
//     pool: &DbPool,
//     id: AssetId
// ) -> Result<FullAsset> {
//         let asset_base: AssetBase = sqlx::query_as!(
//             DbAsset,
//             r#"
// SELECT id, ty as "ty: _", root_dir_id, file_path, file_created_at, file_modified_at, thumb_path_jpg, thumb_path_webp FROM Assets
// WHERE id = ?;
//     "#, id.0)
//         // TODO don't collect into vec before mapping
//             .fetch_one(pool)
//             .await.and_then(|r| Ok(r.into()))?;
//     let asset = match asset_base.ty {
//         crate::model::AssetType::Image => {
//
//         },
//         crate::model::AssetType::Video => {
//         },
//     }
// }
