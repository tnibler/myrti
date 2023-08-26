use std::path::Path;

use chrono::{DateTime, Utc};
use color_eyre::eyre;
use eyre::{eyre, Context, Result};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use tracing::{debug, error, instrument, Instrument};

use crate::model::util::path_to_string;
use crate::model::{
    Asset, AssetId, AssetPathOnDisk, AssetSpe, AssetThumbnails, AssetType, VideoAsset,
};

use super::db_entity::{DbAsset, DbAssetPathOnDisk, DbAssetThumbnails, DbAssetType};
use super::pool::DbPool;

#[instrument(skip(pool))]
pub async fn get_asset(pool: &DbPool, id: AssetId) -> Result<Asset> {
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
rotation_correction as "rotation_correction: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
codec_name,
bitrate,
resource_dir
FROM Asset
WHERE id=?;
    "#,
        id
    )
    .fetch_one(pool)
    .in_current_span()
    .await?
    .try_into()
}

#[instrument(skip(pool))]
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

#[instrument(skip(pool))]
pub async fn asset_with_path_exists(pool: &DbPool, path: &Path) -> Result<bool> {
    let path = path.to_str().unwrap();
    Ok(sqlx::query!(
        r#"
SELECT (1) as a
FROM Asset WHERE file_path = ?;
    "#,
        path
    )
    .fetch_optional(pool)
    .in_current_span()
    .await?
    .is_some())
}

#[instrument(skip(pool))]
pub async fn get_assets(pool: &DbPool) -> Result<Vec<Asset>> {
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
rotation_correction as "rotation_correction: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
codec_name,
bitrate,
resource_dir
FROM Asset;
    "#
    )
    // TODO don't collect into vec before mapping
    .fetch_all(pool)
    .in_current_span()
    .await?
    .into_iter()
    .map(|a| a.try_into())
    .collect()
}

#[instrument(skip(pool))]
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

#[instrument(skip(conn))]
pub async fn update_asset(conn: &mut SqliteConnection, asset: &Asset) -> Result<()> {
    assert!(asset.base.id.0 != 0);
    let db_asset: DbAsset = asset.try_into()?;
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
width=?,
height=?,
rotation_correction=?,
thumb_small_square_avif=?,
thumb_small_square_webp=?,
thumb_large_orig_avif=?,
thumb_large_orig_webp=?,
thumb_small_square_width=?,
thumb_small_square_height=?,
thumb_large_orig_width=?,
thumb_large_orig_height=?,
codec_name=?,
bitrate=?,
resource_dir=?
WHERE id=?;
",
        db_asset.ty,
        db_asset.root_dir_id.0,
        db_asset.file_path,
        db_asset.hash,
        db_asset.added_at,
        db_asset.taken_date,
        db_asset.taken_date_local_fallback,
        db_asset.width,
        db_asset.height,
        db_asset.rotation_correction,
        db_asset.thumb_small_square_avif,
        db_asset.thumb_small_square_webp,
        db_asset.thumb_large_orig_avif,
        db_asset.thumb_large_orig_webp,
        db_asset.thumb_small_square_width,
        db_asset.thumb_small_square_height,
        db_asset.thumb_large_orig_width,
        db_asset.thumb_large_orig_height,
        db_asset.codec_name,
        db_asset.bitrate,
        db_asset.resource_dir,
        asset.base.id.0
    )
    .execute(conn)
    .in_current_span()
    .await
    .wrap_err("could not update table Asset")?;
    Ok(())
}

#[instrument(skip(pool))]
pub async fn insert_asset(pool: &DbPool, asset: &Asset) -> Result<AssetId> {
    if asset.base.id.0 != 0 {
        error!("attempting to insert Asset with non-zero id");
        return Err(eyre!("attempting to insert Asset with non-zero id"));
    }
    if asset.base.ty
        != match asset.sp {
            AssetSpe::Image(_) => AssetType::Image,
            AssetSpe::Video(_) => AssetType::Video,
        }
    {
        error!("attempting to insert Asset with mismatching type and sp fields");
        return Err(eyre!(
            "attempting to insert Asset with mismatching type and sp fields"
        ));
    }
    let db_asset: DbAsset = asset.try_into()?;
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
rotation_correction,
thumb_small_square_avif,
thumb_small_square_webp,
thumb_large_orig_avif,
thumb_large_orig_webp,
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
codec_name,
bitrate,
resource_dir
)
VALUES
(null, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
",
        db_asset.ty,
        db_asset.root_dir_id.0,
        db_asset.file_path,
        db_asset.hash,
        db_asset.added_at,
        db_asset.taken_date,
        db_asset.taken_date_local_fallback,
        db_asset.width,
        db_asset.height,
        db_asset.rotation_correction,
        db_asset.thumb_small_square_avif,
        db_asset.thumb_small_square_webp,
        db_asset.thumb_large_orig_avif,
        db_asset.thumb_large_orig_webp,
        db_asset.thumb_small_square_width,
        db_asset.thumb_small_square_height,
        db_asset.thumb_large_orig_width,
        db_asset.thumb_large_orig_height,
        db_asset.codec_name,
        db_asset.bitrate,
        db_asset.resource_dir
    )
    .execute(pool)
    .in_current_span()
    .await
    .wrap_err("could not insert into table Assets")?;
    let rowid = result.last_insert_rowid();
    Ok(AssetId(rowid))
}

#[instrument(skip(conn))]
pub async fn set_asset_small_thumbnails(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    thumb_small_square_avif: &Path,
    thumb_small_square_webp: &Path,
) -> Result<()> {
    let thumb_small_square_avif = path_to_string(thumb_small_square_avif)?;
    let thumb_small_square_webp = path_to_string(thumb_small_square_webp)?;
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

#[instrument(skip(conn))]
pub async fn set_asset_large_thumbnails(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    thumb_large_orig_avif: &Path,
    thumb_large_orig_webp: &Path,
) -> Result<()> {
    let thumb_large_orig_avif = path_to_string(thumb_large_orig_avif)?;
    let thumb_large_orig_webp = path_to_string(thumb_large_orig_webp)?;
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

#[instrument(skip(pool), level = "debug")]
pub async fn get_video_assets_without_dash(pool: &DbPool) -> Result<Vec<VideoAsset>> {
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
rotation_correction as "rotation_correction: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
codec_name,
bitrate,
resource_dir
FROM Asset 
WHERE
Asset.ty = ?
AND resource_dir IS NULL;
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

#[instrument(skip(pool))]
pub async fn get_asset_timeline_chunk(
    pool: &DbPool,
    start: &DateTime<Utc>,
    start_id: Option<AssetId>,
    count: i32,
) -> Result<Vec<Asset>> {
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
taken_date as "taken_date: _",
taken_date_local_fallback as "taken_date_local_fallback: _",
width,
height,
rotation_correction as "rotation_correction: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
codec_name,
bitrate,
resource_dir
FROM Asset 
WHERE
(taken_date IS NOT NULL AND taken_date < ?) 
OR 
-- TODO can we even lexicographically compare local fallback and DateTime<Utc>
-- no we can't FIXME
(taken_date_local_fallback IS NOT NULL AND taken_date_local_fallback < ?)
AND (id < ? OR ? IS NULL)
ORDER BY taken_date DESC, taken_date_local_fallback DESC, id DESC
LIMIT ?;
    "#,
        // TODO only sort by canonical_date and id when canonical is actually computed during indexing
        start_naive,
        start_naive,
        start_id,
        start_id,
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
) -> Result<Vec<VideoAsset>> {
    let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
WITH codecs AS 
(
    SELECT Asset.id as id, codec_name 
    FROM Asset 
    WHERE Asset.ty=2
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
Asset.rotation_correction as rotation_correction,
Asset.thumb_small_square_avif as thumb_small_square_avif,
Asset.thumb_small_square_webp as thumb_small_square_webp,
Asset.thumb_large_orig_avif as thumb_large_orig_avif,
Asset.thumb_large_orig_webp as thumb_large_orig_webp,
Asset.thumb_small_square_width as thumb_small_square_width,
Asset.thumb_small_square_height as thumb_small_square_height,
Asset.thumb_large_orig_width as thumb_large_orig_width,
Asset.thumb_large_orig_height as thumb_large_orig_height,
Asset.codec_name as codec_name,
Asset.bitrate as bitrate,
Asset.resource_dir as resource_dir
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

#[instrument(skip(pool, acceptable_codecs))]
pub async fn get_videos_in_acceptable_codec_without_dash(
    pool: &DbPool,
    acceptable_codecs: impl Iterator<Item = &str>,
) -> Result<Vec<VideoAsset>> {
    let mut query_builder = QueryBuilder::new(
        r#"
SELECT 
id,
ty,
root_dir_id,
file_path,
hash,
added_at,
taken_date,
taken_date_local_fallback,
width,
height,
rotation_correction,
thumb_small_square_avif,
thumb_small_square_webp,
thumb_large_orig_avif,
thumb_large_orig_webp,
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
codec_name,
bitrate,
resource_dir
FROM Asset 
WHERE resource_dir IS NULL
AND Asset.ty ="#,
    );
    query_builder.push_bind(DbAssetType::Video);
    query_builder.push(
        r#"
AND codec_name IN
    "#,
    );
    query_builder.push_tuples(acceptable_codecs, |mut b, s| {
        b.push_bind(s);
    });
    query_builder
        .build_query_as::<DbAsset>()
        .fetch_all(pool)
        .in_current_span()
        .await
        .wrap_err("could not query for Video Assets with no DASH version")?
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<_>>>()
}
