use std::borrow::Cow;

use camino::Utf8Path as Path;
use chrono::Utc;
use color_eyre::eyre;
use diesel::dsl::sql;
use diesel::sql_types::Bool;
use diesel::{insert_into, prelude::*};
use eyre::{Context, Result};
use tracing::instrument;

use crate::model::{
    self, Asset, AssetId, AssetPathOnDisk, AssetRootDirId, AssetThumbnail, AssetThumbnailId,
    AssetType, CreateAsset, CreateAssetSpe, TimestampInfo, VideoAsset,
};
use crate::model::{
    repository::db_entity::{to_db_asset_ty, DbAssetPathOnDisk, DbAssetThumbnail},
    util::{bool_to_int, datetime_to_db_repr, hash_u64_to_vec8, to_db_thumbnail_type},
};

use super::db::DbConn;
use super::db_entity::{to_db_timezone_info, DbAsset, DbInsertAsset};
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetHasThumbnails {
    pub asset_id: AssetId,
    pub thumbnails: Vec<AssetThumbnail>,
}

#[instrument(skip(conn))]
pub fn get_assets_with_missing_thumbnail(
    conn: &mut DbConn,
    limit: Option<i64>,
) -> Result<Vec<AssetHasThumbnails>> {
    #[derive(Debug, Clone, Queryable, Selectable)]
    #[diesel(table_name = super::schema::Asset)]
    #[diesel(check_for_backend(diesel::sqlite::Sqlite))]
    struct AssetIdRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        pub asset_id: i64,
    }
    let rows_zero_thumbnails: Vec<AssetIdRow> = {
        use diesel::dsl::{exists, not};
        use schema::{Asset, AssetThumbnail};
        Asset::table
            .filter(not(exists(
                AssetThumbnail::table.filter(AssetThumbnail::asset_id.eq(Asset::asset_id)),
            )))
            .select(AssetIdRow::as_select())
            .load(conn)
            .wrap_err("error querying for Assets with zero thumbnails")?
    };
    const NUM_THUMBNAILS_PER_ASSET: i32 = 4;

    let rows_missing_thumbnails: Vec<DbAssetThumbnail> = diesel::sql_query(
        r#"
    SELECT * FROM AssetThumbnail
    WHERE 
    (
        SELECT COUNT(*) FROM AssetThumbnail at
        WHERE at.asset_id = AssetThumbnail.asset_id
        GROUP BY at.asset_id
    ) < $1
    ORDER BY AssetThumbnail.asset_id;
    "#,
    )
    .bind::<diesel::sql_types::Integer, _>(NUM_THUMBNAILS_PER_ASSET)
    .load(conn)?;

    let result_zero_thumbnails = rows_zero_thumbnails
        .into_iter()
        .map(|row| AssetHasThumbnails {
            asset_id: AssetId(row.asset_id),
            thumbnails: Vec::default(),
        });

    let result_missing_thumbnails = rows_missing_thumbnails
        .into_iter()
        .map(AssetThumbnail::try_from)
        .collect::<Result<Vec<AssetThumbnail>>>()?
        .into_iter()
        .fold(
            Vec::default(),
            |mut acc: Vec<AssetHasThumbnails>, thumbnail| {
                match acc.last_mut() {
                    None => {
                        acc.push(AssetHasThumbnails {
                            asset_id: thumbnail.asset_id,
                            thumbnails: vec![thumbnail],
                        });
                    }
                    Some(a) if a.asset_id == thumbnail.asset_id => a.thumbnails.push(thumbnail),
                    Some(a) => {
                        debug_assert!(a.asset_id != thumbnail.asset_id);
                        acc.push(AssetHasThumbnails {
                            asset_id: thumbnail.asset_id,
                            thumbnails: vec![thumbnail],
                        });
                    }
                };
                acc
            },
        );
    Ok(result_zero_thumbnails
        .chain(result_missing_thumbnails)
        .collect())
}

#[instrument(skip(conn))]
pub fn get_thumbnails_for_asset(
    conn: &mut DbConn,
    asset_id: AssetId,
) -> Result<Vec<AssetThumbnail>> {
    use schema::AssetThumbnail;
    let rows: Vec<DbAssetThumbnail> = AssetThumbnail::table
        .filter(AssetThumbnail::asset_id.eq(asset_id.0))
        .select(DbAssetThumbnail::as_select())
        .get_results(conn)?;
    rows.into_iter()
        .map(|r| r.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn))]
pub fn create_asset(conn: &mut DbConn, create_asset: CreateAsset) -> Result<AssetId> {
    let timezone_offset: Option<_> = match create_asset.base.timestamp_info {
        TimestampInfo::TzCertain(tz)
        | TimestampInfo::TzSetByUser(tz)
        | TimestampInfo::TzInferredLocation(tz)
        | TimestampInfo::TzGuessedLocal(tz) => Some(Cow::Owned(tz.to_string())),
        TimestampInfo::UtcCertain | TimestampInfo::NoTimestamp => None,
    };
    let insertable: DbInsertAsset = DbInsertAsset {
        asset_id: None,
        ty: to_db_asset_ty(match &create_asset.spe {
            CreateAssetSpe::Image(_) => AssetType::Image,
            CreateAssetSpe::Video(_) => AssetType::Video,
        }),
        root_dir_id: create_asset.base.root_dir_id.0,
        file_type: create_asset.base.file_type.into(),
        file_path: create_asset.base.file_path.as_str().into(),
        is_hidden: bool_to_int(false),
        hash: create_asset
            .base
            .hash
            .map(|h| Cow::Owned(h.to_le_bytes().to_vec())),
        added_at: datetime_to_db_repr(&Utc::now()),
        taken_date: datetime_to_db_repr(&create_asset.base.taken_date),
        timezone_offset,
        timezone_info: to_db_timezone_info(&create_asset.base.timestamp_info),
        width: create_asset.base.size.width,
        height: create_asset.base.size.height,
        rotation_correction: create_asset.base.rotation_correction,
        exiftool_output: Cow::Borrowed(&create_asset.base.exiftool_output),
        gps_latitude: create_asset.base.gps_coordinates.map(|c| c.lat),
        gps_longitude: create_asset.base.gps_coordinates.map(|c| c.lon),

        motion_photo: 0,
        motion_photo_assoc_asset_id: None,
        motion_photo_pts_us: None,
        motion_photo_video_file_id: None,

        image_format_name: match &create_asset.spe {
            CreateAssetSpe::Image(img) => Some(img.image_format_name.as_str().into()),
            CreateAssetSpe::Video(_) => None,
        },
        ffprobe_output: match &create_asset.spe {
            CreateAssetSpe::Image(_) => None,
            CreateAssetSpe::Video(video) => Some(video.ffprobe_output.0.as_slice().into()),
        },
        video_codec_name: match &create_asset.spe {
            CreateAssetSpe::Image(_) => None,
            CreateAssetSpe::Video(video) => Some(video.video_codec_name.as_str().into()),
        },
        video_bitrate: match &create_asset.spe {
            CreateAssetSpe::Image(_) => None,
            CreateAssetSpe::Video(video) => Some(video.video_bitrate),
        },
        video_duration_ms: match &create_asset.spe {
            CreateAssetSpe::Image(_) => None,
            CreateAssetSpe::Video(video) => video.video_duration_ms,
        },
        audio_codec_name: match &create_asset.spe {
            CreateAssetSpe::Image(_) => None,
            CreateAssetSpe::Video(video) => Some(
                video
                    .audio_codec_name
                    .as_ref()
                    .map(|a| Cow::Borrowed(a.as_str())),
            )
            .flatten(),
        },
        has_dash: match &create_asset.spe {
            CreateAssetSpe::Image(_) => None,
            CreateAssetSpe::Video(video) => Some(bool_to_int(video.has_dash)),
        },
    };
    let id: i64 = insert_into(schema::Asset::table)
        .values(&insertable)
        .returning(schema::Asset::asset_id)
        .get_result(conn)
        .wrap_err("error inserting Asset")?;
    Ok(AssetId(id))
}

#[instrument(skip(conn))]
pub fn insert_asset_thumbnail(
    conn: &mut DbConn,
    thumbnail: AssetThumbnail,
) -> Result<AssetThumbnailId> {
    use schema::AssetThumbnail;
    let id: i64 = diesel::insert_into(AssetThumbnail::table)
        .values((
            AssetThumbnail::asset_id.eq(thumbnail.asset_id.0),
            AssetThumbnail::ty.eq(to_db_thumbnail_type(thumbnail.ty)),
            AssetThumbnail::width.eq(thumbnail.size.width),
            AssetThumbnail::height.eq(thumbnail.size.height),
            AssetThumbnail::format_name.eq(thumbnail.format.to_string()),
        ))
        .returning(AssetThumbnail::thumbnail_id)
        .get_result(conn)
        .wrap_err("error inserting into table AssetThumbnail")?;
    Ok(AssetThumbnailId(id))
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
pub fn get_asset_exiftool_output(conn: &mut DbConn, asset_id: AssetId) -> Result<Vec<u8>> {
    use schema::Asset;
    let exiftool_output: Vec<u8> = Asset::table
        .filter(Asset::asset_id.eq(asset_id.0))
        .select(Asset::exiftool_output)
        .get_result(conn)
        .wrap_err("error querying table Asset")?;
    Ok(exiftool_output)
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
