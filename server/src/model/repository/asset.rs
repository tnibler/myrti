use camino::Utf8Path as Path;
use chrono::{DateTime, SubsecRound, Utc};
use color_eyre::eyre;
use eyre::{eyre, Context, Result};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use tracing::{debug, error, instrument, Instrument};

use crate::model::util::hash_u64_to_vec8;
use crate::model::{
    Asset, AssetBase, AssetId, AssetPathOnDisk, AssetRootDirId, AssetSpe, AssetThumbnails,
    AssetType, CreateAsset, VideoAsset,
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
file_type,
hash,
is_hidden,
added_at,
taken_date,
timezone_offset,
timezone_info as "timezone_info: _",
width,
height,
rotation_correction as "rotation_correction: _",
gps_latitude as "gps_latitude: _",
gps_longitude as "gps_longitude: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
image_format_name,
video_codec_name,
video_bitrate,
audio_codec_name,
has_dash
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

pub async fn get_asset_with_hash(pool: &DbPool, hash: u64) -> Result<Option<AssetId>> {
    let hash = hash_u64_to_vec8(hash);
    let maybe_id = sqlx::query!(
        r#"
SELECT id FROM Asset 
WHERE hash = ?;
    "#,
        hash
    )
    .fetch_optional(pool)
    .await
    .wrap_err("could not query table Asset")?
    .map(|r| AssetId(r.id));
    Ok(maybe_id)
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
pub async fn asset_or_duplicate_with_path_exists(
    pool: &DbPool,
    asset_root_dir_id: AssetRootDirId,
    path: &Path,
) -> Result<bool> {
    let path = path.as_str();
    Ok(sqlx::query!(
        r#"
SELECT (1) as a
FROM Asset WHERE 
file_path = $1 
AND root_dir_id = $2
UNION 
SELECT (1) as a
FROM DuplicateAsset WHERE
file_path = $1
AND root_dir_id = $2
LIMIT 1;
    "#,
        path,
        asset_root_dir_id,
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
file_type,
hash,
is_hidden,
added_at,
taken_date,
timezone_offset,
timezone_info as "timezone_info: _",
width,
height,
rotation_correction as "rotation_correction: _",
gps_latitude as "gps_latitude: _",
gps_longitude as "gps_longitude: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
image_format_name,
video_codec_name,
video_bitrate,
audio_codec_name,
has_dash as "has_dash: _"
FROM Asset;
    "#
    )
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
thumb_small_square_avif,
thumb_small_square_webp,
thumb_large_orig_avif,
thumb_large_orig_webp 
FROM Asset
WHERE   
    thumb_small_square_avif = 0 OR
    thumb_small_square_webp = 0 OR
    thumb_large_orig_avif = 0 OR
    thumb_large_orig_webp = 0
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
    thumb_small_square_avif = 0 OR
    thumb_small_square_webp = 0 OR
    thumb_large_orig_avif = 0 OR
    thumb_large_orig_webp = 0;
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
    let has_dash: Option<i64> = db_asset.has_dash.map(|d| d.into());
    let result = sqlx::query!(
        r#"
INSERT INTO Asset
(id,
ty,
root_dir_id,
file_path,
file_type,
hash,
is_hidden,
added_at,
taken_date,
timezone_offset,
timezone_info,
width,
height,
rotation_correction,
gps_latitude,
gps_longitude,
thumb_small_square_avif,
thumb_small_square_webp,
thumb_large_orig_avif,
thumb_large_orig_webp,
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
image_format_name,
video_codec_name,
video_bitrate,
audio_codec_name,
has_dash 
)
VALUES
(null, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
"#,
        db_asset.ty,
        db_asset.root_dir_id.0,
        db_asset.file_path,
        db_asset.file_type,
        db_asset.hash,
        db_asset.is_hidden,
        db_asset.added_at,
        db_asset.taken_date,
        db_asset.timezone_offset,
        db_asset.timezone_info,
        db_asset.width,
        db_asset.height,
        db_asset.rotation_correction,
        db_asset.gps_latitude,
        db_asset.gps_longitude,
        db_asset.thumb_small_square_avif,
        db_asset.thumb_small_square_webp,
        db_asset.thumb_large_orig_avif,
        db_asset.thumb_large_orig_webp,
        db_asset.thumb_small_square_width,
        db_asset.thumb_small_square_height,
        db_asset.thumb_large_orig_width,
        db_asset.thumb_large_orig_height,
        db_asset.image_format_name,
        db_asset.video_codec_name,
        db_asset.video_bitrate,
        db_asset.audio_codec_name,
        has_dash
    )
    .execute(pool)
    .in_current_span()
    .await
    .wrap_err("could not insert into table Assets")?;
    let rowid = result.last_insert_rowid();
    Ok(AssetId(rowid))
}

#[instrument(skip(pool))]
pub async fn create_asset(pool: &DbPool, create_asset: CreateAsset) -> Result<AssetId> {
    if create_asset.ty
        != match create_asset.sp {
            AssetSpe::Image(_) => AssetType::Image,
            AssetSpe::Video(_) => AssetType::Video,
        }
    {
        error!("attempting to insert Asset with mismatching type and sp fields");
        return Err(eyre!(
            "attempting to insert Asset with mismatching type and sp fields"
        ));
    }
    let asset_base = AssetBase {
        id: AssetId(0),
        ty: create_asset.ty,
        root_dir_id: create_asset.root_dir_id,
        file_type: create_asset.file_type,
        file_path: create_asset.file_path,
        is_hidden: false,
        added_at: Utc::now().trunc_subsecs(3), // db stores milliseconds only
        taken_date: create_asset.taken_date,
        timestamp_info: create_asset.timestamp_info,
        size: create_asset.size,
        rotation_correction: create_asset.rotation_correction,
        gps_coordinates: create_asset.gps_coordinates,
        hash: create_asset.hash,
        thumb_small_square_avif: false,
        thumb_small_square_webp: false,
        thumb_large_orig_avif: false,
        thumb_large_orig_webp: false,
        thumb_large_orig_size: None,
        thumb_small_square_size: None,
    };
    let asset = Asset {
        base: asset_base,
        sp: create_asset.sp,
    };
    insert_asset(pool, &asset).await
}

#[instrument(skip(conn))]
pub async fn set_asset_small_thumbnails(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    thumb_small_square_avif: bool,
    thumb_small_square_webp: bool,
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

#[instrument(skip(conn))]
pub async fn set_asset_has_dash(
    conn: &mut SqliteConnection,
    asset_id: AssetId,
    has_dash: bool,
) -> Result<()> {
    sqlx::query!(
        r#"
UPDATE ASSET SET
has_dash=?
WHERE id=?;
    "#,
        has_dash,
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
    thumb_large_orig_avif: bool,
    thumb_large_orig_webp: bool,
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

#[instrument(skip(pool), level = "debug")]
pub async fn get_video_assets_without_dash(pool: &DbPool) -> Result<Vec<VideoAsset>> {
    sqlx::query_as!(
        DbAsset,
        r#"
SELECT 
id,
ty as "ty: _",
root_dir_id,
file_type,
file_path,
hash,
is_hidden,
added_at,
taken_date,
timezone_offset,
timezone_info as "timezone_info: _",
width,
height,
rotation_correction as "rotation_correction: _",
gps_latitude as "gps_latitude: _",
gps_longitude as "gps_longitude: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
image_format_name,
video_codec_name,
video_bitrate,
audio_codec_name,
has_dash as "has_dash: _"
FROM Asset 
WHERE
Asset.ty = ?
AND has_dash = 0;
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
file_type,
file_path,
hash,
is_hidden,
added_at,
taken_date,
timezone_offset,
timezone_info as "timezone_info: _",
width,
height,
rotation_correction as "rotation_correction: _",
gps_latitude as "gps_latitude: _",
gps_longitude as "gps_longitude: _",
thumb_small_square_avif as "thumb_small_square_avif: _",
thumb_small_square_webp as "thumb_small_square_webp: _",
thumb_large_orig_avif as "thumb_large_orig_avif: _",
thumb_large_orig_webp as "thumb_large_orig_webp: _",
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
image_format_name,
video_codec_name,
video_bitrate,
audio_codec_name,
has_dash as "has_dash: _"
FROM Asset 
WHERE
taken_date < ? 
AND (id < ? OR ? IS NULL)
ORDER BY taken_date DESC, id DESC
LIMIT ?;
    "#,
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

#[instrument(skip(pool, acceptable_video_codecs, acceptable_audio_codecs))]
pub async fn get_video_assets_with_no_acceptable_repr(
    pool: &DbPool,
    acceptable_video_codecs: impl Iterator<Item = &str>,
    acceptable_audio_codecs: impl Iterator<Item = &str>,
) -> Result<Vec<VideoAsset>> {
    let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
WITH
acceptable_audio_codecs AS 
   (
   SELECT * FROM (
        "#,
    );
    // FIXME dirty, just need to make sure there's at least one el ("") in both iterators
    let mut acceptable_audio_codecs: Vec<&str> = acceptable_audio_codecs.into_iter().collect();
    acceptable_audio_codecs.push("");
    let mut acceptable_video_codecs: Vec<&str> = acceptable_video_codecs.into_iter().collect();
    acceptable_video_codecs.push("");
    query_builder.push_values(acceptable_audio_codecs.into_iter(), |mut b, s| {
        b.push(format!("'{}'", s));
    });
    query_builder.push(
        r#"
   )
   ),
acceptable_video_codecs AS 
   (
   SELECT * FROM (
    "#,
    );
    query_builder.push_values(acceptable_video_codecs.into_iter(), |mut b, s| {
        b.push(format!("'{}'", s));
    });
    query_builder.push(
        r#"
   )
   )
SELECT 
Asset.id as id,
Asset.ty as ty,
Asset.root_dir_id as root_dir_id,
Asset.file_type as file_type,
Asset.file_path as file_path,
Asset.hash as hash,
Asset.is_hidden as is_hidden,
Asset.added_at as added_at,
Asset.taken_date as taken_date,
Asset.timezone_offset as timezone_offset,
timezone_info ,
Asset.width as width,
Asset.height as height,
Asset.rotation_correction as rotation_correction,
Asset.gps_latitude as gps_latitude,
Asset.gps_longitude as gps_longitude,
Asset.thumb_small_square_avif as thumb_small_square_avif,
Asset.thumb_small_square_webp as thumb_small_square_webp,
Asset.thumb_large_orig_avif as thumb_large_orig_avif,
Asset.thumb_large_orig_webp as thumb_large_orig_webp,
Asset.thumb_small_square_width as thumb_small_square_width,
Asset.thumb_small_square_height as thumb_small_square_height,
Asset.thumb_large_orig_width as thumb_large_orig_width,
Asset.thumb_large_orig_height as thumb_large_orig_height,
Asset.image_format_name as image_format_name,
Asset.video_codec_name as video_codec_name,
Asset.video_bitrate as video_bitrate,
Asset.audio_codec_name as audio_codec_name,
Asset.has_dash as has_dash
FROM Asset
WHERE Asset.ty =
    "#,
    );
    query_builder.push_bind(DbAssetType::Video);
    query_builder.push(
        r#"
    AND
    (
      (Asset.audio_codec_name IS NOT NULL
      AND
      NOT EXISTS (SELECT * FROM
      (
         SELECT Asset.audio_codec_name
         UNION
         SELECT ar.codec_name FROM AudioRepresentation ar
         WHERE ar.asset_id = Asset.id
         INTERSECT SELECT * FROM acceptable_audio_codecs
      )))
      OR
      (
      NOT EXISTS (SELECT * FROM
      (
         SELECT Asset.video_codec_name
         UNION
         SELECT vr.codec_name FROM VideoRepresentation vr
         WHERE vr.asset_id = Asset.id
         INTERSECT
         SELECT * FROM acceptable_video_codecs
      ))
      )
    );
             "#,
    );
    // panic!("{}", query_builder.sql());
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

#[instrument(skip(pool, acceptable_video_codecs, acceptable_audio_codecs))]
pub async fn get_videos_in_acceptable_codec_without_dash(
    pool: &DbPool,
    acceptable_video_codecs: impl IntoIterator<Item = &str>,
    acceptable_audio_codecs: impl IntoIterator<Item = &str>,
) -> Result<Vec<VideoAsset>> {
    let mut query_builder = QueryBuilder::new(
        r#"
SELECT 
id,
ty,
root_dir_id,
file_type,
file_path,
hash,
is_hidden,
added_at,
taken_date,
timezone_offset,
timezone_info,
width,
height,
rotation_correction,
gps_latitude,
gps_longitude,
thumb_small_square_avif,
thumb_small_square_webp,
thumb_large_orig_avif,
thumb_large_orig_webp,
thumb_small_square_width,
thumb_small_square_height,
thumb_large_orig_width,
thumb_large_orig_height,
image_format_name,
video_codec_name,
video_bitrate,
audio_codec_name,
has_dash
FROM Asset 
WHERE has_dash = 0
AND Asset.ty ="#,
    );
    query_builder.push_bind(DbAssetType::Video);
    query_builder.push(
        r#"
AND video_codec_name IN
    "#,
    );
    query_builder.push_tuples(acceptable_video_codecs.into_iter(), |mut b, s| {
        b.push_bind(s);
    });
    query_builder.push(
        r#"
AND audio_codec_name IN
    "#,
    );
    query_builder.push_tuples(acceptable_audio_codecs.into_iter(), |mut b, s| {
        b.push_bind(s);
    });
    query_builder.push(";");
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

#[instrument(skip(pool, acceptable_codecs))]
pub async fn get_image_assets_with_no_acceptable_repr(
    pool: &DbPool,
    acceptable_codecs: impl Iterator<Item = &str> + Clone,
) -> Result<Vec<AssetId>> {
    let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
SELECT 
Asset.id as id
FROM Asset
WHERE Asset.ty = "#,
    );
    query_builder.push_bind(DbAssetType::Image);
    query_builder.push(
        r#"
AND Asset.image_format_name NOT IN
    "#,
    );
    query_builder.push_tuples(acceptable_codecs.clone(), |mut b, s| {
        b.push_bind(s);
    });
    query_builder.push(
        r#"
AND NOT EXISTS
(
    SELECT (1) FROM ImageRepresentation ir
    WHERE 
    ir.asset_id = Asset.id
    AND ir.format_name IN
    "#,
    );
    query_builder.push_tuples(acceptable_codecs, |mut b, s| {
        b.push_bind(s);
    });
    query_builder.push(
        r#"
);
    "#,
    );
    // #[derive(Debug, sqlx::FromRow)]
    struct Row {
        pub asset_id: AssetId,
    }
    Ok(query_builder
        .build_query_as::<AssetId>()
        .fetch_all(pool)
        .in_current_span()
        .await
        .wrap_err("could not query for Image Assets with no acceptable representations")?
        .into_iter()
        .collect::<Vec<_>>())
}
