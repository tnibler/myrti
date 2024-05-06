use std::borrow::Cow;

use camino::Utf8Path as Path;
use chrono::Utc;
use color_eyre::eyre;
use diesel::dsl::sql;
use diesel::sql_types::Bool;
use diesel::{insert_into, prelude::*};
use eyre::{eyre, Context, Result};
use tracing::{error, instrument};

use crate::model::repository::db_entity::{to_db_asset_ty, AsInsertableAsset, DbAssetPathOnDisk};
use crate::model::util::{bool_to_int, hash_u64_to_vec8, to_db_thumbnail_type};
use crate::model::{
    self, Asset, AssetBase, AssetId, AssetPathOnDisk, AssetRootDirId, AssetSpe, AssetThumbnails,
    AssetType, CreateAsset, CreateAssetSpe, Image, Size, ThumbnailFormat, ThumbnailType, Video,
    VideoAsset,
};

use super::db::DbConn;
use super::db_entity::DbAsset;
use super::schema;

#[instrument(skip(conn))]
pub fn get_asset(conn: &mut DbConn, id: AssetId) -> Result<Asset> {
    use schema::Asset::dsl::*;
    let db_asset: DbAsset = Asset.select(DbAsset::as_select()).find(id.0).first(conn)?;
    db_asset.try_into()
}

#[instrument(skip(conn))]
pub fn get_asset_with_hash(conn: &mut DbConn, with_hash: u64) -> Result<Option<AssetId>> {
    use schema::Asset::dsl::*;
    let with_hash = hash_u64_to_vec8(with_hash);
    let maybe_id: Option<i64> = Asset
        .select(asset_id)
        .filter(hash.eq(Some(with_hash)))
        .first(conn)
        .optional()?;
    Ok(maybe_id.map(AssetId))
}

#[instrument(skip(conn))]
pub fn get_asset_path_on_disk(conn: &mut DbConn, id: AssetId) -> Result<AssetPathOnDisk> {
    use schema::Asset;
    use schema::AssetRootDir;
    let asset: DbAssetPathOnDisk = Asset::table
        .inner_join(AssetRootDir::table)
        .filter(Asset::asset_id.eq(id.0))
        .select((Asset::asset_id, Asset::file_path, AssetRootDir::path))
        .first(conn)?;
    Ok(AssetPathOnDisk {
        id: AssetId(asset.asset_id),
        path_in_asset_root: asset.path_in_asset_root.into(),
        asset_root_path: asset.asset_root_path.into(),
    })
}

#[instrument(skip(conn))]
pub fn asset_or_duplicate_with_path_exists(
    conn: &mut DbConn,
    asset_root_dir_id: AssetRootDirId,
    path: &Path,
) -> Result<bool> {
    use diesel::sql_types::Integer;
    use schema::Asset;
    use schema::DuplicateAsset;
    let path = path.to_string();
    let r: Vec<_> = Asset::table
        .filter(
            Asset::root_dir_id
                .eq(asset_root_dir_id.0)
                .and(Asset::file_path.eq(&path)),
        )
        .select(1.into_sql::<Integer>())
        .limit(1)
        .union(
            DuplicateAsset::table
                .filter(
                    DuplicateAsset::root_dir_id
                        .eq(asset_root_dir_id.0)
                        .and(DuplicateAsset::file_path.eq(&path)),
                )
                .select(1.into_sql::<Integer>())
                .limit(1),
        )
        .load::<i32>(conn)?;
    Ok(!r.is_empty())
}

#[instrument(skip(conn))]
pub fn get_assets(conn: &mut DbConn) -> Result<Vec<Asset>> {
    use schema::Asset::dsl::*;
    let db_assets: Vec<DbAsset> = Asset.select(DbAsset::as_select()).load(conn)?;
    db_assets
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<_>>>()
}

#[derive(Debug, Clone, QueryableByName)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct DbAssetMissingThumbnails {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub asset_id: i64,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub lg_orig_missing: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub sm_sq_missing: i32,
}

#[instrument(skip(conn))]
pub fn get_assets_with_missing_thumbnail(
    conn: &mut DbConn,
    limit: Option<i64>,
) -> Result<Vec<AssetThumbnails>> {
    let rows: Vec<DbAssetMissingThumbnails> = diesel::sql_query(
        r#"
    WITH lg_orig_missing AS 
    (
        SELECT Asset.asset_id, (COUNT(at0.thumbnail_id) < $1) AS missing
        FROM Asset LEFT OUTER JOIN
          (
          SELECT AssetThumbnail.asset_id, thumbnail_id FROM `AssetThumbnail` WHERE ty = $2
          ) at0
        ON Asset.asset_id = at0.asset_id
        GROUP BY Asset.asset_id
    ),
    sm_sq_missing AS 
    (
        SELECT Asset.asset_id, (COUNT(at1.thumbnail_id) < $1) AS missing
        FROM Asset LEFT OUTER JOIN
          (
          SELECT AssetThumbnail.asset_id, thumbnail_id FROM `AssetThumbnail` WHERE ty=$3
          ) at1
        ON Asset.asset_id=at1.asset_id
        GROUP BY Asset.asset_id
    )
    SELECT 
    lg_orig_missing.asset_id AS asset_id,
    lg_orig_missing.missing AS lg_orig_missing,
    sm_sq_missing.missing AS sm_sq_missing
    FROM lg_orig_missing 
    LEFT OUTER JOIN sm_sq_missing
    ON lg_orig_missing.asset_id = sm_sq_missing.asset_id
    WHERE lg_orig_missing.missing != 0 OR sm_sq_missing != 0;
"#,
    )
    // number of thumbnails per type we want. we could have only some formats for a thumbnail
    // missing, but we don't really handle that here. Instead consider only assets with 0
    // thumbnails as missing
    .bind::<diesel::sql_types::Integer, _>(1)
    .bind::<diesel::sql_types::Integer, _>(to_db_thumbnail_type(ThumbnailType::LargeOrigAspect))
    .bind::<diesel::sql_types::Integer, _>(to_db_thumbnail_type(ThumbnailType::SmallSquare))
    .load(conn)
    .wrap_err("error querying for assets with missing thumbnails")?;
    let res = rows
        .into_iter()
        .map(|row| AssetThumbnails {
            id: AssetId(row.asset_id),
            has_thumb_large_orig: row.lg_orig_missing != 0,
            has_thumb_small_square: row.sm_sq_missing != 0,
        })
        .collect();
    Ok(res)
}

#[derive(Debug, Clone, QueryableByName)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct DbAssetHasThumbnails {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub asset_id: i64,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub has_lg_orig: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub has_sm_sq: i32,
}

#[instrument(skip(conn))]
pub fn get_thumbnails_for_asset(conn: &mut DbConn, asset_id: AssetId) -> Result<AssetThumbnails> {
    let row: DbAssetHasThumbnails = diesel::sql_query(r#"
SELECT Asset.asset_id AS asset_id, (lg_orig.thumbnail_id IS NOT NULL) as has_lg_orig, (sm_sq.thumbnail_id IS NOT NULL) AS has_sm_sq
FROM Asset
LEFT OUTER JOIN AssetThumbnail lg_orig ON (Asset.asset_id = lg_orig.asset_id AND lg_orig.ty = ?)
LEFT OUTER JOIN AssetThumbnail sm_sq ON (Asset.asset_id = sm_sq.asset_id AND sm_sq.ty = ?)
WHERE 
Asset.asset_id = ?;
    "#)
        .bind::<diesel::sql_types::BigInt, _>(asset_id.0)
        .bind::<diesel::sql_types::Integer, _>(to_db_thumbnail_type(ThumbnailType::LargeOrigAspect))
        .bind::<diesel::sql_types::Integer, _>(to_db_thumbnail_type(ThumbnailType::SmallSquare))
        .get_result(conn)
        .wrap_err("error querying for asset thumbnails")?;
    Ok(AssetThumbnails {
        id: AssetId(row.asset_id),
        has_thumb_large_orig: row.has_lg_orig != 0,
        has_thumb_small_square: row.has_sm_sq != 0,
    })
}

#[instrument(skip(conn, ffprobe_output))]
#[deprecated = "use create_asset instead"]
#[doc(hidden)]
/// Only really used for tests an in create_asset
/// ffprobe_output is passed separately because adding it as a field to DbAsset would mean we have
/// to query and pass it around everywhere, even though it's almost never needed.
/// Later big tables like Asset could probably be divided into important and less important fields
/// and spread over two different struct types for sqlx queries
pub fn insert_asset(
    conn: &mut DbConn,
    asset: &Asset,
    ffprobe_output: Option<impl AsRef<[u8]>>,
) -> Result<AssetId> {
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
    let ffprobe_output: Option<&[u8]> = ffprobe_output.as_ref().map(|o| o.as_ref());

    let insertable = asset.as_insertable(ffprobe_output.map(Cow::Borrowed));
    let id: i64 = insert_into(schema::Asset::table)
        .values(&insertable)
        .returning(schema::Asset::asset_id)
        .get_result(conn)
        .wrap_err("error inserting Asset")?;
    Ok(AssetId(id))
}

#[instrument(skip(conn))]
pub fn create_asset(conn: &mut DbConn, create_asset: CreateAsset) -> Result<AssetId> {
    let (ty, sp, ffprobe_output) = match create_asset.spe {
        CreateAssetSpe::Image(image) => (
            AssetType::Image,
            AssetSpe::Image(Image {
                image_format_name: image.image_format_name.clone(),
            }),
            None,
        ),
        CreateAssetSpe::Video(video) => (
            AssetType::Video,
            AssetSpe::Video(Video {
                video_codec_name: video.video_codec_name.clone(),
                video_bitrate: video.video_bitrate,
                audio_codec_name: video.audio_codec_name.clone(),
                has_dash: video.has_dash,
            }),
            Some(video.ffprobe_output),
        ),
    };
    let asset_base = AssetBase {
        id: AssetId(0),
        ty,
        root_dir_id: create_asset.base.root_dir_id,
        file_type: create_asset.base.file_type,
        file_path: create_asset.base.file_path,
        is_hidden: false,
        added_at: Utc::now(), // db stores milliseconds only
        taken_date: create_asset.base.taken_date,
        timestamp_info: create_asset.base.timestamp_info,
        size: create_asset.base.size,
        rotation_correction: create_asset.base.rotation_correction,
        gps_coordinates: create_asset.base.gps_coordinates,
        hash: create_asset.base.hash,
    };
    let asset = Asset {
        base: asset_base,
        sp,
    };

    let insertable = asset.as_insertable(ffprobe_output.map(|o| Cow::Owned(o.0)));
    let id: i64 = insert_into(schema::Asset::table)
        .values(&insertable)
        .returning(schema::Asset::asset_id)
        .get_result(conn)
        .wrap_err("error inserting Asset")?;
    Ok(AssetId(id))
}

#[instrument(skip(conn))]
pub fn set_asset_has_thumbnail(
    conn: &mut DbConn,
    asset_id: AssetId,
    ty: ThumbnailType,
    size: Size,
    formats: &[ThumbnailFormat],
) -> Result<()> {
    // This whole thumbnail logic is weirdly in between flexible and basically hardcoded
    // to only ever insert 1 pair of webp/avif thumbnails in both sqaure and original aspect ratios.
    // If thumbnail formats ever become configurable or something, all this can be actually
    // implemented.
    use schema::AssetThumbnail;
    conn.transaction(|conn| {
        for format in formats {
            let format_name = match format {
                ThumbnailFormat::Webp => "webp",
                ThumbnailFormat::Avif => "avif",
            };
            diesel::insert_into(AssetThumbnail::table)
                .values((
                    AssetThumbnail::asset_id.eq(asset_id.0),
                    AssetThumbnail::ty.eq(to_db_thumbnail_type(ty)),
                    AssetThumbnail::width.eq(size.width),
                    AssetThumbnail::height.eq(size.height),
                    AssetThumbnail::format_name.eq(format_name),
                ))
                .execute(conn)?;
        }
        Ok::<_, eyre::Report>(())
    })
    .wrap_err("error committing transaction inserting AssetThumbnail rows")?;
    Ok(())
}

#[instrument(skip(conn))]
pub fn set_asset_has_dash(conn: &mut DbConn, asset_id: AssetId, has_dash: bool) -> Result<()> {
    use schema::Asset;
    diesel::update(Asset::table.find(asset_id.0))
        .set(Asset::has_dash.eq(bool_to_int(has_dash)))
        .execute(conn)?;
    Ok(())
}

#[instrument(skip(conn))]
pub fn get_video_assets_without_dash(conn: &mut DbConn) -> Result<Vec<VideoAsset>> {
    use schema::Asset::dsl::*;
    let db_assets: Vec<DbAsset> = Asset
        .filter(
            ty.eq(to_db_asset_ty(AssetType::Video))
                .and(has_dash.eq(bool_to_int(false))),
        )
        .select(DbAsset::as_select())
        .load(conn)?;
    db_assets
        .into_iter()
        .map(|db_asset| VideoAsset::try_from(model::Asset::try_from(db_asset)?))
        .collect::<Result<Vec<VideoAsset>>>()
}

#[instrument(skip(conn))]
pub fn get_video_assets_with_no_acceptable_repr(conn: &mut DbConn) -> Result<Vec<VideoAsset>> {
    use schema::Asset;
    let query = Asset::table
        .select(DbAsset::as_select())
        .filter(Asset::ty.eq(to_db_asset_ty(AssetType::Video)))
        .filter(sql::<Bool>(
            r#"
            (
            (
                Asset.audio_codec_name IS NOT NULL
                AND
                NOT EXISTS 
                (
                    SELECT * FROM
                    (
                        SELECT Asset.audio_codec_name
                        UNION
                        SELECT ar.codec_name FROM AudioRepresentation ar
                        WHERE ar.asset_id = Asset.asset_id
                    )
                    INTERSECT SELECT * FROM AcceptableAudioCodec
                )
            )
            OR
            (
                NOT EXISTS
                (
                    SELECT * FROM
                    (
                        SELECT * FROM (SELECT Asset.video_codec_name WHERE Asset.file_type='mp4')
                        UNION
                        SELECT vr.codec_name FROM VideoRepresentation vr
                        WHERE vr.asset_id = Asset.asset_id
                    )
                    INTERSECT SELECT * FROM AcceptableVideoCodec
                )
            )
            )
        "#,
        ));
    let db_assets: Vec<DbAsset> = query.load(conn)?;
    db_assets
        .into_iter()
        .map(|db_asset| model::Asset::try_from(db_asset)?.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn))]
pub fn get_videos_in_acceptable_codec_without_dash(conn: &mut DbConn) -> Result<Vec<VideoAsset>> {
    use schema::Asset;
    let db_assets: Vec<DbAsset> = Asset::table
        .select(DbAsset::as_select())
        .filter(
            Asset::ty
                .eq(to_db_asset_ty(AssetType::Video))
                .and(Asset::has_dash.assume_not_null().eq(bool_to_int(false)))
                .and(Asset::file_type.eq("mp4")),
        )
        .filter(
            sql::<Bool>(r#"
            (
                Asset.audio_codec_name IS NULL 
                OR 
                EXISTS (SELECT codec_name FROM AcceptableAudioCodec WHERE codec_name = Asset.audio_codec_name)
            ) 
            AND 
                EXISTS (SELECT codec_name FROM AcceptableVideoCodec WHERE codec_name = Asset.video_codec_name)
            "#)
        )
        .load(conn)?;
    db_assets
        .into_iter()
        .map(|db_asset| model::Asset::try_from(db_asset)?.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn, acceptable_codecs))]
pub fn get_image_assets_with_no_acceptable_repr(
    conn: &mut DbConn,
    acceptable_codecs: &[&str],
) -> Result<Vec<AssetId>> {
    use diesel::dsl::{exists, not};
    use schema::{Asset, ImageRepresentation};
    let asset_ids: Vec<i64> = Asset::table
        .filter(Asset::ty.eq(to_db_asset_ty(AssetType::Image)))
        .filter(not(Asset::image_format_name
            .assume_not_null()
            .eq_any(acceptable_codecs)))
        .filter(not(exists(
            ImageRepresentation::table.filter(
                ImageRepresentation::asset_id
                    .eq(Asset::asset_id)
                    .and(ImageRepresentation::format_name.eq_any(acceptable_codecs)),
            ),
        )))
        .select(Asset::asset_id)
        .load(conn)?;
    Ok(asset_ids.into_iter().map(AssetId).collect())
}

#[instrument(skip(conn))]
pub fn get_ffprobe_output(conn: &mut DbConn, asset_id: AssetId) -> Result<Vec<u8>> {
    use schema::Asset;
    let ffprobe_output: Vec<u8> = Asset::table
        .find(asset_id.0)
        .select(Asset::ffprobe_output.assume_not_null())
        .first(conn)?;
    Ok(ffprobe_output)
}

#[instrument(skip(conn))]
pub fn set_assets_hidden(conn: &mut DbConn, set_hidden: bool, asset_ids: &[AssetId]) -> Result<()> {
    use schema::Asset;
    diesel::update(Asset::table.filter(Asset::asset_id.eq_any(asset_ids.iter().map(|id| id.0))))
        .set(Asset::is_hidden.eq(bool_to_int(set_hidden)))
        .execute(conn)
        .wrap_err("error updating column Asset.is_hidden")?;
    Ok(())
}

#[instrument(skip(conn))]
pub fn set_asset_rotation_correction(
    conn: &mut DbConn,
    asset_id: AssetId,
    rotation: Option<i32>,
) -> Result<()> {
    use schema::Asset;
    diesel::update(Asset::table.filter(Asset::asset_id.eq(asset_id.0)))
        .set(Asset::rotation_correction.eq(rotation))
        .execute(conn)
        .wrap_err("error updating column Asset.rotation_correction")?;
    Ok(())
}
