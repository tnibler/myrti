use chrono::{DateTime, Utc};
use color_eyre::eyre;
use eyre::{bail, Context, Result};
use sqlx::{query, QueryBuilder, Sqlite, SqliteConnection};
use std::path::Path;
use tracing::{debug, instrument, Instrument};

use super::db_entity::{DbAsset, DbAssetPathOnDisk, DbAssetThumbnails, DbAssetType, DbVideoInfo};
use crate::model::{
    AssetAll, AssetBase, AssetId, AssetPathOnDisk, AssetThumbnails, AssetType, FullAsset, Image,
    ResourceFileId, ThumbnailType, Video,
};

use super::pool::DbPool;

#[instrument(name = "Get AssetBase", skip(pool), level = "debug")]
pub async fn get_asset_base(pool: &DbPool, asset_id: AssetId) -> Result<AssetBase> {
    debug_assert!(asset_id.0 != 0);
    let db_asset = sqlx::query_as!(
        DbAsset,
        r#"
SELECT
id,
ty as "ty: _",
root_dir_id,
file_path,
hash,
added_at,
taken_date as "taken_date: _",
taken_date_local_fallback as "taken_date_local_fallback: _",
width,
height,
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height
FROM Asset
WHERE id=?;
"#,
        asset_id
    )
    .fetch_optional(pool)
    .in_current_span()
    .await
    .wrap_err("could not query table Assets")?;
    match db_asset {
        Some(a) => Ok(a.try_into()?),
        None => bail!("no Asset with this id {}", asset_id),
    }
}

#[instrument(name = "Get AssetBases", skip(pool), level = "debug")]
pub async fn get_asset_bases(pool: &DbPool, ids: &[AssetId]) -> Vec<Result<AssetBase>> {
    let mut results = Vec::<Result<AssetBase>>::new();
    for id in ids {
        results.push(get_asset_base(pool, *id).await);
    }
    results
}

#[instrument(name = "Get Asset path on disk", skip(pool), level = "debug")]
pub async fn get_asset_path_on_disk(pool: &DbPool, id: AssetId) -> Result<AssetPathOnDisk> {
    sqlx::query_as!(
        DbAssetPathOnDisk,
        r#"
SELECT 
Asset.id AS id,
Asset.file_path AS path_in_asset_root,
AssetRootDir.path AS asset_root_path
FROM Asset INNER JOIN AssetRootDir ON Asset.root_dir_id = AssetRootDir.id
WHERE Asset.id = ?;
        "#,
        id.0
    )
    .fetch_one(pool)
    .await
    .map(|r| r.try_into())?
}

#[instrument(name = "Insert FullAsset", skip(pool, asset), level = "debug")]
pub async fn insert_asset(pool: &DbPool, asset: FullAsset) -> Result<AssetId> {
    debug_assert!(
        asset.base.ty
            == match asset.asset {
                AssetAll::Image(_) => AssetType::Image,
                AssetAll::Video(_) => AssetType::Video,
            }
    );
    let mut tx = pool.begin().in_current_span().await?;
    let id = insert_asset_base(&mut tx, &asset.base)
        .in_current_span()
        .await?;
    match &asset.asset {
        AssetAll::Image(image) => {
            insert_image_info(&mut tx, id, image)
                .in_current_span()
                .await?;
        }
        AssetAll::Video(video) => {
            insert_video_info(&mut tx, id, video)
                .in_current_span()
                .await?;
        }
    };
    tx.commit().in_current_span().await?;
    Ok(id)
}

// #[instrument(name = "Update FullAsset", skip(pool, asset), fields(id=%asset.base.id), level = "debug")]
// pub async fn update_asset(pool: &DbPool, asset: FullAsset) -> Result<()> {
//     debug_assert!(
//         asset.base.ty
//             == match asset.asset {
//                 AssetAll::Image(_) => AssetType::Image,
//                 AssetAll::Video(_) => AssetType::Video,
//             }
//     );
//     let mut tx = pool.begin().in_current_span().await?;
//     update_asset_base(&mut tx, &asset.base)
//         .in_current_span()
//         .await?;
//     let id = asset.base.id;
//     match &asset.asset {
//         AssetAll::Image(image) => {
//             update_image_info(&mut tx, id, image)
//                 .in_current_span()
//                 .await?;
//         }
//         AssetAll::Video(video) => {
//             update_video_info(&mut tx, id, video)
//                 .in_current_span()
//                 .await?;
//         }
//     };
//     tx.commit().in_current_span().await?;
//     Ok(())
// }

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
taken_date as "taken_date: _",
taken_date_local_fallback as "taken_date_local_fallback: _",
width,
height,
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height
FROM Asset WHERE file_path = ?;
    "#,
        path
    )
    .fetch_optional(pool)
    .in_current_span()
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
taken_date as "taken_date: _",
taken_date_local_fallback as "taken_date_local_fallback: _",
width,
height,
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height
FROM Asset;
    "#
    )
    // TODO don't collect into vec before mapping
    .fetch_all(pool)
    .in_current_span()
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
) -> Result<Vec<AssetThumbnails>> {
    if let Some(limit) = limit {
        sqlx::query_as!(
            DbAssetThumbnails,
            r#"
SELECT id,
ty as "ty: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _"
FROM Asset
WHERE   
    thumb_small_square_avif IS NULL OR
    thumb_small_square_webp IS NULL OR
    thumb_large_orig_avif IS NULL OR
    thumb_large_orig_avif IS NULL
LIMIT ?;
    "#,
            limit
        )
        .fetch_all(pool)
        .in_current_span()
        .await?
        .into_iter()
        .map(|r| r.try_into())
        .collect()
    } else {
        sqlx::query_as!(
            DbAssetThumbnails,
            r#"
SELECT id,
ty as "ty: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _"
FROM Asset
WHERE   
    thumb_small_square_avif IS NULL OR
    thumb_small_square_webp IS NULL OR
    thumb_large_orig_avif IS NULL OR
    thumb_large_orig_avif IS NULL;
    "#
        )
        .fetch_all(pool)
        .in_current_span()
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
UPDATE Asset SET 
ty=?,
root_dir_id=?,
file_path=?,
hash=?,
added_at=?,
taken_date=?,
taken_date_local_fallback=?,
thumb_small_square_avif=?,
thumb_small_square_webp=?,
thumb_large_orig_avif=?,
thumb_large_orig_webp=?
WHERE id=?;
",
        db_asset_base.ty,
        db_asset_base.root_dir_id.0,
        db_asset_base.file_path,
        db_asset_base.hash,
        db_asset_base.added_at,
        db_asset_base.taken_date,
        db_asset_base.taken_date_local_fallback,
        db_asset_base.thumb_small_square_avif,
        db_asset_base.thumb_small_square_webp,
        db_asset_base.thumb_large_orig_avif,
        db_asset_base.thumb_large_orig_webp,
        asset.id.0
    )
    .execute(conn)
    .in_current_span()
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
INSERT INTO Asset
(id,
ty,
root_dir_id,
file_path,
hash,
added_at,
taken_date,
taken_date_local_fallback,
width,
height,
thumb_small_square_avif,
thumb_small_square_webp,
thumb_large_orig_avif,
thumb_large_orig_webp,
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height
)
VALUES
(null, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
",
        db_asset_base.ty,
        db_asset_base.root_dir_id.0,
        db_asset_base.file_path,
        db_asset_base.hash,
        db_asset_base.added_at,
        db_asset_base.taken_date,
        db_asset_base.taken_date_local_fallback,
        db_asset_base.width,
        db_asset_base.height,
        db_asset_base.thumb_small_square_avif,
        db_asset_base.thumb_small_square_webp,
        db_asset_base.thumb_large_orig_avif,
        db_asset_base.thumb_large_orig_webp,
        db_asset_base.thumb_small_square_width,
        db_asset_base.thumb_small_square_height,
        db_asset_base.thumb_large_orig_width,
        db_asset_base.thumb_large_orig_height,
    )
    .execute(conn)
    .in_current_span()
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
    let _db_image_info = image.try_to_db_image_info(asset_id)?;
    sqlx::query!("INSERT INTO ImageInfo (asset_id) VALUES(?);", asset_id.0,)
        .execute(conn)
        .in_current_span()
        .await
        .wrap_err("could not insert into table ImageInfo")?;
    Ok(())
}

// #[instrument(name = "Update ImageInfo", skip(conn, image), level = "debug")]
// pub async fn update_image_info(
//     conn: &mut SqliteConnection,
//     asset_id: AssetId,
//     image: &Image,
// ) -> Result<()> {
//     debug_assert!(asset_id.0 != 0);
//     let db_image_info = image.try_to_db_image_info(asset_id)?;
//     Ok(())
// }

#[instrument(skip(conn, video), level = "debug")]
pub async fn insert_video_info(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    video: &Video,
) -> Result<()> {
    debug_assert!(asset_id.0 != 0);
    let db_video_info: DbVideoInfo = video.try_to_db_video_info(asset_id)?;
    sqlx::query!(
        "
INSERT INTO VideoInfo (asset_id, codec_name, dash_resource_dir) VALUES
(?, ?, ?);
",
        asset_id.0,
        db_video_info.codec_name,
        db_video_info.dash_resource_dir
    )
    .execute(conn)
    .in_current_span()
    .await
    .wrap_err("could not insert into table VideoInfo")?;
    Ok(())
}

#[instrument(skip(conn, video), level = "debug")]
pub async fn update_video_info(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    video: &Video,
) -> Result<()> {
    debug_assert!(asset_id.0 != 0);
    let db_video_info: DbVideoInfo = video.try_to_db_video_info(asset_id)?;
    sqlx::query!(
        "
UPDATE VideoInfo SET 
codec_name=? ,
dash_resource_dir=?
WHERE asset_id=?;
",
        db_video_info.codec_name,
        db_video_info.dash_resource_dir,
        asset_id.0
    )
    .execute(conn)
    .in_current_span()
    .await
    .wrap_err("could not update table VideoInfo")?;
    Ok(())
}

#[instrument(skip(conn), level = "debug")]
pub async fn set_asset_small_thumbnails(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    thumb_small_square_avif: ResourceFileId,
    thumb_small_square_webp: ResourceFileId,
) -> Result<()> {
    sqlx::query!(
        r#"
UPDATE Asset SET 
thumb_small_square_avif=?,
thumb_small_square_webp=?
WHERE id=?;
    "#,
        thumb_small_square_avif,
        thumb_small_square_webp,
        asset_id
    )
    .execute(conn)
    .await
    .wrap_err("could not update table Assets")?;
    Ok(())
}

#[instrument(
    name = "Update Asset, set large thumbnails",
    skip(conn),
    level = "debug"
)]
pub async fn set_asset_large_thumbnails(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    thumb_large_orig_avif: ResourceFileId,
    thumb_large_orig_webp: ResourceFileId,
) -> Result<()> {
    sqlx::query!(
        r#"
UPDATE Asset SET 
thumb_large_orig_avif=?,
thumb_large_orig_webp=?
WHERE id=?;
    "#,
        thumb_large_orig_avif,
        thumb_large_orig_webp,
        asset_id
    )
    .execute(conn)
    .await
    .wrap_err("could not update table Assets")?;
    Ok(())
}

#[instrument(skip(conn))]
pub async fn set_asset_thumbnail(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    thumbnail_type: ThumbnailType,
    avif: ResourceFileId,
    webp: ResourceFileId,
) -> Result<()> {
    let query = match thumbnail_type {
        ThumbnailType::SmallSquare => {
            sqlx::query!(
                r#"
UPDATE Asset SET 
thumb_small_square_avif=?,
thumb_small_square_webp=?
WHERE id=?;
    "#,
                avif,
                webp,
                asset_id
            )
        }
        ThumbnailType::LargeOrigAspect => {
            sqlx::query!(
                r#"
UPDATE Asset SET 
thumb_large_orig_avif=?,
thumb_large_orig_webp=?
WHERE id=?;
    "#,
                avif,
                webp,
                asset_id
            )
        }
    };
    query
        .execute(conn)
        .await
        .wrap_err("could not update table Assets")?;
    Ok(())
}

#[instrument(skip(pool), level = "debug")]
pub async fn get_video_assets_without_dash(pool: &DbPool) -> Result<Vec<AssetBase>> {
    sqlx::query_as!(
        DbAsset,
        r#"
SELECT 
id,
ty as "ty: _",
root_dir_id,
file_path,
hash,
added_at,
taken_date as "taken_date: _",
taken_date_local_fallback as "taken_date_local_fallback: _",
width,
height,
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height
FROM Asset, VideoInfo 
WHERE Asset.id = VideoInfo.asset_id 
AND Asset.ty = ? AND VideoInfo.dash_resource_dir IS NULL;
    "#,
        DbAssetType::Video
    )
    .fetch_all(pool)
    .in_current_span()
    .await
    .wrap_err("could not fetch video assets without mpd manifest from db")?
    .into_iter()
    .map(|a| a.try_into())
    .collect::<Result<Vec<_>>>()
}

#[instrument(name = "Get VideoInfo for asset", skip(pool), level = "debug")]
pub async fn get_video_info(pool: &DbPool, asset_id: AssetId) -> Result<Video> {
    sqlx::query_as!(
        DbVideoInfo,
        r#"
SELECT 
asset_id,
codec_name,
dash_resource_dir as "dash_resource_dir: _"
FROM VideoInfo WHERE asset_id=?;
    "#,
        asset_id
    )
    .fetch_one(pool)
    .in_current_span()
    .await
    .wrap_err("no VideoInfo for this AssetId")?
    .try_into()
}

#[instrument(skip(pool))]
pub async fn get_asset_timeline_chunk(
    pool: &DbPool,
    start: &DateTime<Utc>,
    count: i32,
) -> Result<Vec<AssetBase>> {
    let start_naive = start.naive_utc();
    sqlx::query_as!(
        DbAsset,
        r#"
SELECT
id,
ty as "ty: _",
root_dir_id,
file_path,
hash,
added_at,
taken_date,
taken_date_local_fallback,
width,
height,
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height
FROM Asset 
WHERE
(taken_date IS NOT NULL AND taken_date < ?) 
OR 
-- TODO can we even lexicographically compare local fallback and DateTime<Utc>
-- no we can't FIXME
(taken_date_local_fallback IS NOT NULL AND taken_date_local_fallback < ?)
ORDER BY taken_date DESC, taken_date_local_fallback DESC, id DESC
LIMIT ?;
    "#,
        // TODO only sort by canonical_date and id when canonical is actually computed during indexing
        start_naive,
        start_naive,
        count
    )
    .fetch_all(pool)
    .in_current_span()
    .await
    .wrap_err("could not query for timeline chunk")?
    .into_iter()
    .map(|a| a.try_into())
    .collect::<Result<Vec<_>>>()
}

#[instrument(skip(pool, acceptable_codecs))]
pub async fn get_video_assets_with_no_acceptable_repr(
    pool: &DbPool,
    acceptable_codecs: impl Iterator<Item = &str>,
) -> Result<Vec<AssetBase>> {
    // WITH codecs AS
    // (SELECT Asset.id as id, codec_name
    // FROM Asset, VideoInfo
    // WHERE Asset.ty=2 AND Asset.id=VideoInfo.asset_id
    // UNION
    // SELECT Asset.id as id, vr.codec_name
    // FROM Asset, VideoRepresentation vr
    // WHERE Asset.id=vr.asset_id)
    // SELECT * FROM Asset
    // WHERE Asset.ty=2
    // AND id NOT IN
    // (SELECT id FROM codecs WHERE codec_name IN ("h264", "av1"));
    let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
WITH codecs AS 
(
    SELECT Asset.id as id, codec_name 
    FROM Asset, VideoInfo 
    WHERE Asset.ty=2 AND Asset.id=VideoInfo.asset_id 
    UNION 
    SELECT Asset.id as id, vr.codec_name 
    FROM Asset, VideoRepresentation vr 
    WHERE Asset.id=vr.asset_id
) 

SELECT 
Asset.id as id,
Asset.ty as ty,
Asset.root_dir_id as root_dir_id,
Asset.file_path as file_path,
Asset.hash as hash,
Asset.added_at as added_at,
Asset.taken_date as taken_date,
Asset.taken_date_local_fallback as taken_date_local_fallback,
Asset.width as width,
Asset.height as height,
Asset.thumb_small_square_avif as thumb_small_square_avif,
Asset.thumb_small_square_webp as thumb_small_square_webp,
Asset.thumb_large_orig_avif as thumb_large_orig_avif,
Asset.thumb_large_orig_webp as thumb_large_orig_webp,
Asset.thumb_small_square_width as thumb_small_square_width,
Asset.thumb_small_square_height as thumb_small_square_height,
Asset.thumb_large_orig_width as thumb_large_orig_width,
Asset.thumb_large_orig_height as thumb_large_orig_height
FROM Asset
WHERE Asset.ty = "#,
    );
    query_builder.push_bind(DbAssetType::Video);
    query_builder.push(
        r#"
AND id NOT IN
    (SELECT id FROM codecs WHERE codec_name IN 
    "#,
    );
    query_builder.push_tuples(acceptable_codecs, |mut b, s| {
        b.push_bind(s);
    });
    query_builder.push(");");
    debug!(query = query_builder.sql());
    query_builder
        .build_query_as::<DbAsset>()
        .fetch_all(pool)
        .in_current_span()
        .await
        .wrap_err("could not query for Video Assets with no acceptable representations")?
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<_>>>()
}
