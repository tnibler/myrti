use std::borrow::Cow;

use camino::Utf8Path as Path;
use chrono::Utc;
use color_eyre::eyre;
use diesel::connection::SimpleConnection;
use diesel::{insert_into, prelude::*};
use eyre::{Context, Result, eyre};
use itertools::Itertools;
use tracing::instrument;

use crate::model::repository::db_entity::{
    DbAssetFile, DbImageFile, DbInsertAssetFile, DbInsertVideoFile, DbVideoFile,
};
use crate::model::{
    self, Asset, AssetBase, AssetFile, AssetId, AssetPathOnDisk, AssetRootDirId, AssetSpe,
    AssetThumbnail, AssetThumbnailId, AssetType, CreateAsset, CreateAssetSpe, FileId, HasGhiIndex,
    Image, MirrorCorrection, RotationCorrection, ThumbnailType, TimestampInfo, Video,
};
use crate::model::{
    repository::db_entity::{DbAssetPathOnDisk, DbAssetThumbnail, to_db_asset_ty},
    util::{bool_to_int, datetime_to_db_repr, hash_u64_to_vec8, to_db_thumbnail_type},
};

use super::db::DbConn;
use super::db_entity::{DbAsset, DbInsertAsset, to_db_timezone_info};
use super::schema;

pub fn get_asset(conn: &mut DbConn, id: AssetId) -> Result<Asset> {
    use schema::{Asset, AssetFile};
    conn.transaction(|conn| {
        let (db_asset, db_file): (DbAsset, DbAssetFile) = Asset::table
            .find(id.0)
            .inner_join(AssetFile::table.on(AssetFile::file_id.eq(Asset::rep_file_id)))
            .select((DbAsset::as_select(), DbAssetFile::as_select()))
            .first(conn)?;
        let image_asset: Option<DbImageFile> = DbImageFile::belonging_to(&db_file)
            .select(DbImageFile::as_select())
            .get_result(conn)
            .optional()?;
        if let Some(image_asset) = image_asset {
            return Ok(model::Asset {
                base: db_asset.try_into()?,
                sp: AssetSpe::Image(Image {
                    file_id: FileId(db_file.file_id),
                    image_format_name: image_asset.image_format_name,
                }),
                rep_file: db_file.try_into()?,
            });
        }
        let video_asset: Option<DbVideoFile> = DbVideoFile::belonging_to(&db_file)
            .select(DbVideoFile::as_select())
            .get_result(conn)
            .optional()?;
        if let Some(video_asset) = video_asset {
            return Ok(model::Asset {
                base: db_asset.try_into()?,
                rep_file: db_file.try_into()?,
                sp: AssetSpe::Video(video_asset.try_into()?),
            });
        }
        panic!("asset is neither image nor video, db constraints disallow this")
    })
}

pub fn get_asset_file(conn: &mut DbConn, id: FileId) -> Result<model::AssetFile> {
    schema::AssetFile::table
        .find(id.0)
        .select(DbAssetFile::as_select())
        .get_result::<DbAssetFile>(conn)?
        .try_into()
}

pub fn get_video_file(conn: &mut DbConn, id: FileId) -> Result<(model::Video, model::AssetFile)> {
    use schema::{AssetFile, VideoFile};
    let (db_video, db_file) = VideoFile::table
        .find(id.0)
        .inner_join(AssetFile::table)
        .select((DbVideoFile::as_select(), DbAssetFile::as_select()))
        .first::<(DbVideoFile, DbAssetFile)>(conn)?;
    Ok((db_video.try_into()?, db_file.try_into()?))
}

pub fn get_image_file(conn: &mut DbConn, id: FileId) -> Result<(model::Image, model::AssetFile)> {
    use schema::{AssetFile, ImageFile};
    let (db_image, db_file) = ImageFile::table
        .find(id.0)
        .inner_join(AssetFile::table)
        .select((DbImageFile::as_select(), DbAssetFile::as_select()))
        .first::<(DbImageFile, DbAssetFile)>(conn)?;
    Ok((db_image.try_into()?, db_file.try_into()?))
}

pub fn get_asset_with_hash(conn: &mut DbConn, with_hash: u64) -> Result<Option<FileId>> {
    use schema::AssetFile;
    let with_hash = hash_u64_to_vec8(with_hash);
    let maybe_id: Option<i64> = AssetFile::table
        .select(AssetFile::asset_id)
        .filter(AssetFile::hash.eq(Some(with_hash)))
        .first(conn)
        .optional()?;
    Ok(maybe_id.map(FileId))
}

pub fn get_asset_path_on_disk(conn: &mut DbConn, id: FileId) -> Result<AssetPathOnDisk> {
    use schema::AssetFile;
    use schema::AssetRootDir;
    let row: DbAssetPathOnDisk = AssetFile::table
        .inner_join(AssetRootDir::table)
        .filter(AssetFile::file_id.eq(id.0))
        .select((AssetFile::file_id, AssetFile::file_path, AssetRootDir::path))
        .first(conn)?;
    Ok(AssetPathOnDisk {
        file_id: FileId(row.file_id),
        path_in_asset_root: row.path_in_asset_root.into(),
        asset_root_path: row.asset_root_path.into(),
    })
}

pub fn asset_or_duplicate_with_path_exists(
    conn: &mut DbConn,
    asset_root_dir_id: AssetRootDirId,
    path: &Path,
) -> Result<bool> {
    use diesel::sql_types::Integer;
    use schema::AssetFile;
    use schema::DuplicateFile;
    let path = path.to_string();
    let r: Vec<_> = AssetFile::table
        .filter(
            AssetFile::root_dir_id
                .eq(asset_root_dir_id.0)
                .and(AssetFile::file_path.eq(&path)),
        )
        .select(1.into_sql::<Integer>())
        .limit(1)
        .union(
            DuplicateFile::table
                .filter(
                    DuplicateFile::root_dir_id
                        .eq(asset_root_dir_id.0)
                        .and(DuplicateFile::file_path.eq(&path)),
                )
                .select(1.into_sql::<Integer>())
                .limit(1),
        )
        .load::<i32>(conn)?;
    Ok(!r.is_empty())
}

pub fn get_assets(conn: &mut DbConn) -> Result<Vec<Asset>> {
    use schema::{Asset, AssetFile, ImageFile, VideoFile};
    let images = Asset::table
        .inner_join(AssetFile::table.on(Asset::rep_file_id.eq(AssetFile::file_id)))
        .inner_join(ImageFile::table.on(Asset::rep_file_id.eq(ImageFile::file_id)))
        .select((
            DbAsset::as_select(),
            DbAssetFile::as_select(),
            DbImageFile::as_select(),
        ))
        .load(conn)?
        .into_iter()
        .map(|(base, file, image): (DbAsset, DbAssetFile, DbImageFile)| {
            Ok(model::Asset {
                base: base.try_into()?,
                rep_file: file.try_into()?,
                sp: AssetSpe::Image(image.try_into()?),
            })
        });
    let videos = Asset::table
        .inner_join(AssetFile::table.on(Asset::rep_file_id.eq(AssetFile::file_id)))
        .inner_join(VideoFile::table.on(Asset::rep_file_id.eq(VideoFile::file_id)))
        .select((
            DbAsset::as_select(),
            DbAssetFile::as_select(),
            DbVideoFile::as_select(),
        ))
        .load(conn)?
        .into_iter()
        .map(|(base, file, video): (DbAsset, DbAssetFile, DbVideoFile)| {
            Ok(model::Asset {
                base: base.try_into()?,
                rep_file: file.try_into()?,
                sp: AssetSpe::Video(video.try_into()?),
            })
        });
    images.chain(videos).collect::<Result<Vec<_>>>()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetHasThumbnails {
    pub file_id: FileId,
    pub thumbnails: Vec<AssetThumbnail>,
}

#[instrument(skip(conn), level = "debug")]
pub fn get_assets_with_missing_thumbnail(
    conn: &mut DbConn,
    limit: Option<i64>,
) -> Result<Vec<AssetHasThumbnails>> {
    #[derive(Debug, Clone, QueryableByName, Selectable)]
    #[diesel(table_name = super::schema::AssetFile)]
    #[diesel(check_for_backend(diesel::sqlite::Sqlite))]
    struct FileIdRow {
        pub file_id: i64,
    }
    const NUM_THUMBNAILS_PER_ASSET: i64 = 4;

    let missing_thumbnails: Vec<(FileIdRow, Option<DbAssetThumbnail>)> = diesel::sql_query(
        r#"
        SELECT AssetFile.file_id, AssetThumbnail.*
        FROM AssetFile
        LEFT JOIN AssetThumbnail ON AssetFile.file_id = AssetThumbnail.file_id
        WHERE AssetFile.file_id NOT IN (
            SELECT at.file_id FROM AssetThumbnail at
            GROUP BY at.file_id
            HAVING COUNT(at.thumbnail_id) >= $1
        )
        ORDER BY AssetFile.file_id;
            "#,
    )
    .bind::<diesel::sql_types::BigInt, _>(NUM_THUMBNAILS_PER_ASSET)
    .load(conn)
    .wrap_err("error querying for AssetFile with missing thumbnails")?;

    let result = missing_thumbnails
        .into_iter()
        .map(|(file_id, db_thumbnail)| {
            Ok((
                FileId(file_id.file_id),
                db_thumbnail
                    .map(model::AssetThumbnail::try_from)
                    .transpose()?,
            ))
        })
        .collect::<Result<Vec<(FileId, Option<model::AssetThumbnail>)>>>()?
        .into_iter()
        .fold(
            Vec::default(),
            |mut acc: Vec<AssetHasThumbnails>, (file_id, thumbnail)| {
                match acc.last_mut() {
                    None => {
                        acc.push(AssetHasThumbnails {
                            file_id,
                            thumbnails: thumbnail.into_iter().collect(),
                        });
                    }
                    Some(a) if a.file_id == file_id => a.thumbnails.push(thumbnail.expect(
                        "row with null AssetThumbnail can only occur zero or one time per file_id",
                    )),
                    Some(_a) => {
                        acc.push(AssetHasThumbnails {
                            file_id,
                            thumbnails: thumbnail.into_iter().collect(),
                        });
                    }
                };
                acc
            },
        );
    Ok(result)
}

#[instrument(skip(conn), level = "trace")]
pub fn get_thumbnails_for_asset(
    conn: &mut DbConn,
    asset_id: FileId,
) -> Result<Vec<AssetThumbnail>> {
    use schema::AssetThumbnail;
    let rows: Vec<DbAssetThumbnail> = AssetThumbnail::table
        .filter(AssetThumbnail::file_id.eq(asset_id.0))
        .select(DbAssetThumbnail::as_select())
        .get_results(conn)?;
    rows.into_iter()
        .map(|r| r.try_into())
        .collect::<Result<Vec<_>>>()
}

pub fn delete_thumbnails_for_file(conn: &mut DbConn, file_ids: Option<&[FileId]>) -> Result<usize> {
    use schema::AssetThumbnail;
    if let Some(file_ids) = file_ids {
        diesel::delete(AssetThumbnail::table)
            .filter(AssetThumbnail::file_id.eq_any(file_ids.iter().map(|id| id.0)))
            .execute(conn)
            .wrap_err("error deleting from table AssetThumbnail")
    } else {
        diesel::delete(AssetThumbnail::table)
            .execute(conn)
            .wrap_err("error deleting from table AssetThumbnail")
    }
}

#[instrument(skip_all, fields(path=create_asset.base.file_path.as_str()), level = "debug")]
pub fn create_asset(conn: &mut DbConn, create_asset: CreateAsset) -> Result<AssetId> {
    let timezone_offset: Option<_> = match create_asset.base.timestamp_info {
        TimestampInfo::TzCertain(tz)
        | TimestampInfo::TzSetByUser(tz)
        | TimestampInfo::TzInferredLocation(tz)
        | TimestampInfo::TzGuessedLocal(tz) => Some(Cow::Owned(tz.to_string())),
        TimestampInfo::UtcCertain | TimestampInfo::NoTimestamp => None,
    };
    conn.immediate_transaction(|conn| {
        let insertable_file = DbInsertAssetFile {
            file_id: None,
            asset_id: Some(0),
            asset_type: to_db_asset_ty(match &create_asset.spe {
                CreateAssetSpe::Image(_) => AssetType::Image,
                CreateAssetSpe::Video(_) => AssetType::Video,
            }),
            root_dir_id: create_asset.base.root_dir_id.0,
            file_type: create_asset.base.file_type.into(),
            file_path: create_asset.base.file_path.as_str().into(),
            hash: create_asset
                .base
                .hash
                .map(|h| Cow::Owned(h.to_le_bytes().to_vec())),
            added_at: datetime_to_db_repr(&Utc::now()),
            width: create_asset.base.size.width,
            height: create_asset.base.size.height,
            rotation_correction: match create_asset.base.rotation_correction {
                RotationCorrection::CW0 => 0,
                RotationCorrection::CW90 => 1,
                RotationCorrection::CW180 => 2,
                RotationCorrection::CW270 => 3,
            },
            exiftool_output: Cow::Borrowed(&create_asset.base.exiftool_output),
        };
        let file_id: i64 = insert_into(schema::AssetFile::table)
            .values(insertable_file)
            .returning(schema::AssetFile::file_id)
            .get_result(conn)
            .wrap_err("error inserting AssetFile")?;

        let insertable: DbInsertAsset = DbInsertAsset {
            asset_id: None,
            asset_type: to_db_asset_ty(match &create_asset.spe {
                CreateAssetSpe::Image(_) => AssetType::Image,
                CreateAssetSpe::Video(_) => AssetType::Video,
            }),
            rep_file_id: Some(file_id),
            is_hidden: bool_to_int(create_asset.base.is_hidden),
            taken_date: datetime_to_db_repr(&create_asset.base.taken_date),
            timezone_offset,
            timezone_info: to_db_timezone_info(&create_asset.base.timestamp_info),
            gps_latitude: create_asset.base.gps_coordinates.map(|c| c.lat),
            gps_longitude: create_asset.base.gps_coordinates.map(|c| c.lon),
        };
        let asset_id: i64 = insert_into(schema::Asset::table)
            .values(&insertable)
            .returning(schema::Asset::asset_id)
            .get_result(conn)
            .wrap_err("error inserting Asset")?;

        diesel::update(schema::AssetFile::table.find(file_id))
            .set(schema::AssetFile::asset_id.eq(asset_id))
            .execute(conn)
            .wrap_err("Error updating AssetFile.asset_id")?;

        match create_asset.spe {
            CreateAssetSpe::Image(create_asset_image) => {
                let _file_id = insert_into(schema::ImageFile::table)
                    .values((
                        schema::ImageFile::file_id.eq(file_id),
                        schema::ImageFile::image_format_name
                            .eq(&create_asset_image.image_format_name),
                    ))
                    .execute(conn)
                    .wrap_err("error inserting ImageFile")?;
            }
            CreateAssetSpe::Video(create_asset_video) => {
                let _video_file_id = insert_into(schema::VideoFile::table)
                    .values(DbInsertVideoFile {
                        file_id,
                        ffprobe_output: Cow::Borrowed(&create_asset_video.ffprobe_output.0),
                        video_codec_name: create_asset_video.video_codec_name.as_str().into(),
                        video_bitrate: create_asset_video.video_bitrate,
                        video_duration_ms: create_asset_video.video_duration_ms,
                        audio_codec_name: create_asset_video
                            .audio_codec_name
                            .as_deref()
                            .map(Cow::Borrowed),
                        has_ghi: None,
                        is_original_streamable: bool_to_int(
                            create_asset_video.is_original_streamable,
                        ),
                        max_iframe_interval: create_asset_video.max_iframe_interval,
                        frame_rate_num: create_asset_video.frame_rate.map(|(num, _)| num),
                        frame_rate_denom: create_asset_video.frame_rate.map(|(_, denom)| denom),
                    })
                    .execute(conn)
                    .wrap_err("error inserting VideoAsset")?;
            }
        }

        Ok(AssetId(asset_id))
    })
}

#[instrument(skip(conn), level = "trace")]
pub fn insert_asset_thumbnail(
    conn: &mut DbConn,
    thumbnail: AssetThumbnail,
) -> Result<AssetThumbnailId> {
    use schema::AssetThumbnail;
    let id: i64 = diesel::insert_into(AssetThumbnail::table)
        .values((
            AssetThumbnail::file_id.eq(thumbnail.file_id.0),
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

pub fn get_exiftool_output(conn: &mut DbConn, file_id: FileId) -> Result<Vec<u8>> {
    use schema::AssetFile;
    let exiftool_output: Vec<u8> = AssetFile::table
        .filter(AssetFile::file_id.eq(file_id.0))
        .select(AssetFile::exiftool_output)
        .get_result(conn)
        .wrap_err("error querying table AssetFile")?;
    Ok(exiftool_output)
}

#[instrument(skip(conn), level = "debug")]
pub fn get_video_files_with_no_acceptable_audio_repr(
    conn: &mut DbConn,
) -> Result<Vec<(AssetFile, Video)>> {
    let query = diesel::sql_query(
        r#"
            SELECT AssetFile.*, VideoFile.* 
            FROM AssetFile INNER JOIN VideoFile ON AssetFile.file_id = VideoFile.file_id
            WHERE
            VideoFile.audio_codec_name IS NOT NULL
            AND
            NOT EXISTS
            (
                SELECT * FROM
                (
                    SELECT ar.codec_name FROM AudioRepresentation ar WHERE ar.file_id = VideoFile.file_id
                    UNION
                    SELECT VideoFile.audio_codec_name WHERE VideoFile.has_ghi = 2 OR VideoFile.has_ghi = 3
                )
                INTERSECT SELECT * FROM AcceptableAudioCodec
            );
        "#,
    );
    let db_assets: Vec<(DbAssetFile, DbVideoFile)> = query
        .load(conn)
        .wrap_err("error querying for VideoFiles with no acceptable codec representations")?;
    db_assets
        .into_iter()
        .map(|(asset_file, video_file)| {
            Ok((
                model::AssetFile::try_from(asset_file)?,
                model::Video::try_from(video_file)?,
            ))
        })
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn), level = "debug")]
pub fn get_video_files_with_no_acceptable_video_repr(
    conn: &mut DbConn,
) -> Result<Vec<(AssetFile, Video)>> {
    let query = diesel::sql_query(
        r#"
            SELECT AssetFile.*, VideoFile.* 
            FROM AssetFile INNER JOIN VideoFile ON AssetFile.file_id = VideoFile.file_id
            WHERE
            NOT EXISTS
            (
                SELECT * FROM
                (
                    SELECT vr.codec_name FROM VideoRepresentation vr WHERE vr.file_id = VideoFile.file_id
                    UNION
                    SELECT VideoFile.video_codec_name WHERE VideoFile.has_ghi = 1 OR VideoFile.has_ghi = 3
                )
                INTERSECT SELECT * FROM AcceptableVideoCodec
            );
        "#,
    );
    let db_assets: Vec<(DbAssetFile, DbVideoFile)> = query
        .load(conn)
        .wrap_err("error querying for VideoFiles with no acceptable codec representations")?;
    db_assets
        .into_iter()
        .map(|(asset_file, video_file)| {
            Ok((
                model::AssetFile::try_from(asset_file)?,
                model::Video::try_from(video_file)?,
            ))
        })
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn))]
pub fn get_video_files_with_unknown_ghi(conn: &mut DbConn) -> Result<Vec<Video>> {
    use schema::VideoFile;
    let rows: Vec<DbVideoFile> = VideoFile::table
        .filter(VideoFile::has_ghi.is_null())
        .select(DbVideoFile::as_select())
        .load(conn)
        .wrap_err("error querying table VideoFile")?;
    rows.into_iter()
        .map(|row| Video::try_from(row))
        .try_collect()
}

#[instrument(skip(conn, acceptable_codecs), level = "debug")]
pub fn get_image_assets_with_no_acceptable_repr(
    conn: &mut DbConn,
    acceptable_codecs: &[&str],
) -> Result<Vec<FileId>> {
    use diesel::dsl::{exists, not};
    use schema::{ImageFile, ImageRepresentation};
    let file_ids: Vec<i64> = ImageFile::table
        .filter(not(ImageFile::image_format_name.eq_any(acceptable_codecs)))
        .filter(not(exists(
            ImageRepresentation::table.filter(
                ImageRepresentation::file_id
                    .eq(ImageFile::file_id)
                    .and(ImageRepresentation::format_name.eq_any(acceptable_codecs)),
            ),
        )))
        .select(ImageFile::file_id)
        .load(conn)?;
    Ok(file_ids.into_iter().map(FileId).collect())
}

pub fn get_ffprobe_output(conn: &mut DbConn, video_file_id: FileId) -> Result<Vec<u8>> {
    use schema::VideoFile;
    let ffprobe_output: Vec<u8> = VideoFile::table
        .find(video_file_id.0)
        .select(VideoFile::ffprobe_output)
        .first(conn)?;
    Ok(ffprobe_output)
}

pub fn get_files_without_thumbhash(conn: &mut DbConn) -> Result<Vec<FileId>> {
    use schema::{AssetFile, AssetThumbnail};
    let rows = AssetFile::table
        .inner_join(AssetThumbnail::table)
        .filter(
            AssetFile::thumb_hash.is_null().and(
                AssetFile::asset_type
                    .eq(to_db_asset_ty(AssetType::Image))
                    // videos only if they already have usable a thumbnail in original aspect ratio
                    // to create a thumbhash from
                    .or(AssetFile::asset_type
                        .eq(to_db_asset_ty(AssetType::Video))
                        .and(
                            AssetThumbnail::ty
                                .eq(to_db_thumbnail_type(ThumbnailType::LargeOrigAspect)),
                        )),
            ),
        )
        .select(AssetFile::file_id)
        .limit(1000)
        .get_results(conn)
        .wrap_err("error querying table AssetFile")?;
    Ok(rows.into_iter().map(FileId).collect())
}

pub fn set_file_thumbhash(conn: &mut DbConn, file_id: FileId, thumbhash: &str) -> Result<()> {
    use schema::AssetFile;
    let n_affected = diesel::update(AssetFile::table)
        .filter(AssetFile::file_id.eq(file_id.0))
        .set(AssetFile::thumb_hash.eq(thumbhash))
        .execute(conn)
        .wrap_err("error updating column AssetFile.thumb_hash")?;
    if n_affected == 1 {
        Ok(())
    } else {
        Err(eyre!("error updating column AssetFile.thumb_hash"))
    }
}

pub fn set_assets_hidden(conn: &mut DbConn, set_hidden: bool, asset_ids: &[AssetId]) -> Result<()> {
    use schema::Asset;
    diesel::update(Asset::table.filter(Asset::asset_id.eq_any(asset_ids.iter().map(|id| id.0))))
        .set(Asset::is_hidden.eq(bool_to_int(set_hidden)))
        .execute(conn)
        .wrap_err("error updating column Asset.is_hidden")?;
    Ok(())
}

/// Sets rotation for all files belonging to the same Asset
pub fn set_rotation_correction_all_asset_files(
    conn: &mut DbConn,
    file_id: FileId,
    rotation: RotationCorrection,
) -> Result<AssetId> {
    use schema::AssetFile;
    let rotation = match rotation {
        RotationCorrection::CW0 => 0,
        RotationCorrection::CW90 => 1,
        RotationCorrection::CW180 => 2,
        RotationCorrection::CW270 => 3,
    };
    let af2 = diesel::alias!(AssetFile as af2);
    diesel::update(AssetFile::table)
        .filter(
            AssetFile::asset_id.eq_any(af2.find(file_id.0).select(af2.field(AssetFile::asset_id))),
        )
        .set(AssetFile::rotation_correction.eq(rotation))
        .returning(AssetFile::asset_id)
        .get_result(conn)
        .map(AssetId)
        .wrap_err("error updating column AssetFile.rotation_correction")
}

/// Sets mirror for all files belonging to the same Asset
pub fn set_mirror_correction_all_asset_files(
    conn: &mut DbConn,
    file_id: FileId,
    mirror: MirrorCorrection,
) -> Result<AssetId> {
    use schema::AssetFile;
    let mirror = match mirror {
        MirrorCorrection::None => 0,
        MirrorCorrection::Horizontal => 1,
        MirrorCorrection::Vertical => 2,
    };
    let af2 = diesel::alias!(AssetFile as af2);
    diesel::update(AssetFile::table)
        .filter(
            AssetFile::asset_id.eq_any(af2.find(file_id.0).select(af2.field(AssetFile::asset_id))),
        )
        .set(AssetFile::mirror_correction.eq(mirror))
        .returning(AssetFile::asset_id)
        .get_result(conn)
        .map(AssetId)
        .wrap_err("error updating column AssetFile.mirror_correction")
}

#[instrument(skip(conn))]
pub fn set_asset_is_series_selection(
    conn: &mut DbConn,
    asset_id: AssetId,
    is_selection: bool,
) -> Result<()> {
    use schema::Asset;
    let n_affected = diesel::update(
        Asset::table.filter(
            Asset::asset_id
                .eq(asset_id.0)
                .and(Asset::is_series_selection.is_not_null()),
        ),
    )
    .set(Asset::is_series_selection.eq(Some(is_selection as i32)))
    .execute(conn)
    .wrap_err("error updating column Asset.is_series_selection")?;
    if n_affected == 1 {
        Ok(())
    } else {
        Err(eyre!(
            "Could not update Asset.is_series_selection: asset id {asset_id} is not part of any series"
        ))
    }
}

pub fn set_asset_max_iframe_interval(
    conn: &mut DbConn,
    asset_id: AssetId,
    interval: Option<f64>,
) -> Result<()> {
    use schema::Asset;
    todo!()
    // diesel::update(Asset::table.filter(Asset::asset_id.eq(asset_id.0))).set(Asset::h)
}

pub fn get_asset_has_ghi_index(conn: &mut DbConn, file_id: FileId) -> Result<Option<HasGhiIndex>> {
    use schema::VideoFile;
    let r: Option<i32> = VideoFile::table
        .find(file_id.0)
        .select(VideoFile::has_ghi)
        .get_result(conn)
        .context("querying VideoFile for has_ghi_index")?;
    r.map(|r| HasGhiIndex::try_from(r)).transpose()
}

pub fn set_asset_has_ghi_index(
    conn: &mut DbConn,
    file_id: FileId,
    has_ghi_index: HasGhiIndex,
) -> Result<()> {
    use schema::VideoFile;
    conn.immediate_transaction(|conn| {
        let n_affected = diesel::update(VideoFile::table.find(file_id.0))
            .set(VideoFile::has_ghi.eq(i32::from(has_ghi_index)))
            .execute(conn)
            .context("error updating column VideoFile.has_ghi_index")?;
        if n_affected == 1 {
            Ok(())
        } else {
            Err(eyre!("error updating column VideoFile.has_ghi_index"))
        }
    })
}

#[instrument(skip(conn), level = "debug")]
pub fn get_all_assets_geojson(conn: &mut DbConn) -> Result<String> {
    #[derive(Debug, Clone, QueryableByName)]
    #[diesel(check_for_backend(diesel::sqlite::Sqlite))]
    struct Row {
        #[diesel(sql_type = diesel::sql_types::Text)]
        pub point_features: String,
    }
    let geojson: Row = diesel::sql_query(
        r#"
    WITH point_features AS (
        SELECT json_object(
            'type', 'Feature',
            'geometry', json_object(
                'type', 'Point',
                'coordinates', json_array(Asset.gps_longitude * 1e-9, Asset.gps_latitude * 1e-9)
            ),
            'properties', json_object(
                'id', Asset.asset_id,
                'taken_date', Asset.taken_date
            )
        ) AS feature FROM Asset 
        WHERE Asset.gps_latitude IS NOT NULL AND Asset.gps_longitude IS NOT NULL
    ) 
    SELECT json_object('type', 'FeatureCollection', 'features', json_group_array(json(feature))) AS point_features FROM point_features;
        "#,
    )
    .get_result(conn)?;
    Ok(geojson.point_features)
}

#[instrument(skip(conn), level = "debug")]
pub fn merge_image_assets(conn: &mut DbConn) -> Result<()> {
    conn.immediate_transaction(|conn| {
        conn.batch_execute(include_str!("merge_assets.sql"))
            .wrap_err("error executing merge_assets query")
    })
}

#[instrument(skip(conn), level = "debug")]
pub fn detect_image_sequences(conn: &mut DbConn) -> Result<()> {
    conn.immediate_transaction(|conn| {
        conn.batch_execute(include_str!("detect_image_sequences.sql"))
            .wrap_err("error executing detect_image_sequence query")
    })
}
