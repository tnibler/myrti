use std::borrow::Cow;

use camino::Utf8Path as Path;
use chrono::Utc;
use color_eyre::eyre;
use diesel::dsl::{not, sql};
use diesel::sql_types::Bool;
use diesel::{insert_into, prelude::*};
use eyre::{eyre, Context, Result};
use tracing::{error, instrument};

use crate::model::repository::db_entity::{to_db_asset_ty, AsInsertableAsset, DbAssetPathOnDisk};
use crate::model::util::{bool_to_int, hash_u64_to_vec8};
use crate::model::{
    self, Asset, AssetBase, AssetId, AssetPathOnDisk, AssetRootDirId, AssetSpe, AssetThumbnails,
    AssetType, CreateAsset, CreateAssetSpe, Image, Video, VideoAsset,
};

use super::db::DbConn;
use super::db_entity::{from_db_asset_ty, DbAsset};
use super::schema;

#[instrument(skip(conn), level = "trace")]
pub fn get_asset(conn: &mut DbConn, id: AssetId) -> Result<Asset> {
    use schema::Asset::dsl::*;
    let db_asset: DbAsset = Asset.select(DbAsset::as_select()).find(id.0).first(conn)?;
    db_asset.try_into()
}

#[instrument(skip(conn), level = "trace")]
pub fn get_asset_with_hash(conn: &mut DbConn, with_hash: u64) -> Result<Option<AssetId>> {
    use schema::Asset::dsl::*;
    let with_hash = hash_u64_to_vec8(with_hash);
    let maybe_id: Option<i64> = Asset
        .select(asset_id)
        .filter(hash.eq(Some(with_hash)))
        .first(conn)
        .optional()?;
    Ok(maybe_id.map(|id| AssetId(id)))
}

#[instrument(skip(conn), level = "trace")]
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

#[instrument(skip(conn), level = "trace")]
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

#[instrument(skip(conn), level = "trace")]
pub fn get_assets(conn: &mut DbConn) -> Result<Vec<Asset>> {
    use schema::Asset::dsl::*;
    let db_assets: Vec<DbAsset> = Asset.select(DbAsset::as_select()).load(conn)?;
    db_assets
        .into_iter()
        .map(|a| a.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn), level = "trace")]
pub fn get_assets_with_missing_thumbnail(
    conn: &mut DbConn,
    limit: Option<i64>,
) -> Result<Vec<AssetThumbnails>> {
    use schema::Asset::dsl::*;
    let query = Asset
        .filter(
            thumb_small_square_avif
                .eq(0)
                .or(thumb_small_square_avif.eq(0))
                .or(thumb_small_square_webp.eq(0))
                .or(thumb_large_orig_avif.eq(0))
                .or(thumb_large_orig_webp.eq(0)),
        )
        .select((
            asset_id,
            ty,
            thumb_small_square_avif,
            thumb_small_square_webp,
            thumb_large_orig_avif,
            thumb_large_orig_webp,
        ));
    let rows: Vec<(i64, i32, i32, i32, i32, i32)> = match limit {
        Some(limit) => query.limit(limit).load(conn),
        None => query.load(conn),
    }?;
    rows.into_iter()
        .map(|(id, typ, ssa, ssw, loa, low)| {
            Ok(AssetThumbnails {
                id: AssetId(id),
                ty: from_db_asset_ty(typ)?,
                thumb_small_square_avif: ssa != 0,
                thumb_small_square_webp: ssw != 0,
                thumb_large_orig_avif: loa != 0,
                thumb_large_orig_webp: low != 0,
            })
        })
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn, ffprobe_output), level = "trace")]
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

    let insertable = asset.as_insertable(ffprobe_output.map(|o| Cow::Borrowed(o)));
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
        thumb_small_square_avif: false,
        thumb_small_square_webp: false,
        thumb_large_orig_avif: false,
        thumb_large_orig_webp: false,
        thumb_large_orig_size: None,
        thumb_small_square_size: None,
    };
    let asset = Asset {
        base: asset_base,
        sp,
    };

    let insertable = asset.as_insertable(ffprobe_output.map(|o| Cow::Owned(o)));
    let id: i64 = insert_into(schema::Asset::table)
        .values(&insertable)
        .returning(schema::Asset::asset_id)
        .get_result(conn)
        .wrap_err("error inserting Asset")?;
    Ok(AssetId(id))
}

#[instrument(skip(conn), level = "debug")]
pub fn set_asset_small_thumbnails(
    conn: &mut DbConn,
    asset_id: AssetId,
    thumb_small_square_avif: bool,
    thumb_small_square_webp: bool,
) -> Result<()> {
    use schema::Asset;
    diesel::update(Asset::table.find(asset_id.0))
        .set((
            Asset::thumb_small_square_webp.eq(bool_to_int(thumb_small_square_webp)),
            Asset::thumb_small_square_avif.eq(bool_to_int(thumb_small_square_avif)),
        ))
        .execute(conn)?;
    Ok(())
}

#[instrument(skip(conn), level = "debug")]
pub fn set_asset_has_dash(conn: &mut DbConn, asset_id: AssetId, has_dash: bool) -> Result<()> {
    use schema::Asset;
    diesel::update(Asset::table.find(asset_id.0))
        .set(Asset::has_dash.eq(bool_to_int(has_dash)))
        .execute(conn)?;
    Ok(())
}

#[instrument(skip(conn), level = "debug")]
pub fn set_asset_large_thumbnails(
    conn: &mut DbConn,
    asset_id: AssetId,
    thumb_large_orig_avif: bool,
    thumb_large_orig_webp: bool,
) -> Result<()> {
    use schema::Asset;
    diesel::update(Asset::table.find(asset_id.0))
        .set((
            Asset::thumb_large_orig_webp.eq(bool_to_int(thumb_large_orig_webp)),
            Asset::thumb_large_orig_avif.eq(bool_to_int(thumb_large_orig_avif)),
        ))
        .execute(conn)?;
    Ok(())
}

#[instrument(skip(conn), level = "debug")]
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

#[instrument(skip(conn), level = "debug")]
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
    tracing::debug!(?db_assets);
    db_assets
        .into_iter()
        .map(|db_asset| model::Asset::try_from(db_asset)?.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn), level = "trace")]
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
    Ok(asset_ids.into_iter().map(|id| AssetId(id)).collect())
}

#[instrument(skip(conn), level = "trace")]
pub fn get_ffprobe_output(conn: &mut DbConn, asset_id: AssetId) -> Result<Vec<u8>> {
    use schema::Asset;
    let ffprobe_output: Vec<u8> = Asset::table
        .find(asset_id.0)
        .select(Asset::ffprobe_output.assume_not_null())
        .first(conn)?;
    Ok(ffprobe_output)
}
