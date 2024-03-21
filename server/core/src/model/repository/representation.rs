use diesel::prelude::*;
use eyre::{ Context, Result};
use tracing::instrument;

use crate::model::{
    repository::db_entity::{DbAudioRepresentation, DbImageRepresentation, DbVideoRepresentation},
    AssetId, AudioRepresentation, AudioRepresentationId, ImageRepresentation,
    ImageRepresentationId, VideoRepresentation, VideoRepresentationId,
};

use super::db::DbConn;
use super::schema;

#[instrument(skip(conn), level = "trace")]
pub fn get_video_representations(
    conn: &mut DbConn,
    asset_id: AssetId,
) -> Result<Vec<VideoRepresentation>> {
    use schema::VideoRepresentation;
    let db_video_reprs: Vec<DbVideoRepresentation> = VideoRepresentation::table
        .filter(VideoRepresentation::asset_id.eq(asset_id.0))
        .load(conn)?;

    db_video_reprs
        .into_iter()
        .map(|db_vr| db_vr.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn), level = "trace")]
pub fn insert_video_representation(
    conn: &mut DbConn,
    repr: &VideoRepresentation,
) -> Result<VideoRepresentationId> {
    use schema::VideoRepresentation;

    assert!(repr.id.0 == 0);

    let id = diesel::insert_into(VideoRepresentation::table)
        .values((
            VideoRepresentation::asset_id.eq(repr.asset_id.0),
            VideoRepresentation::codec_name.eq(&repr.codec_name),
            VideoRepresentation::width.eq(repr.width),
            VideoRepresentation::height.eq(repr.height),
            VideoRepresentation::bitrate.eq(&repr.bitrate),
            VideoRepresentation::file_key.eq(&repr.file_key),
            VideoRepresentation::media_info_key.eq(&repr.media_info_key),
        ))
        .returning(VideoRepresentation::video_repr_id)
        .get_result(conn)
        .wrap_err("error inserting into table VideoRepresentation")?;
    Ok(VideoRepresentationId(id))
}

#[instrument(skip(conn), level = "trace")]
pub fn insert_audio_representation(
    conn: &mut DbConn,
    repr: &AudioRepresentation,
) -> Result<AudioRepresentationId> {
    use schema::AudioRepresentation;

    assert!(repr.id.0 == 0);

    let id = diesel::insert_into(AudioRepresentation::table)
        .values((
            AudioRepresentation::asset_id.eq(repr.asset_id.0),
            AudioRepresentation::codec_name.eq(&repr.codec_name),
            AudioRepresentation::file_key.eq(&repr.file_key),
            AudioRepresentation::media_info_key.eq(&repr.media_info_key),
        ))
        .returning(AudioRepresentation::audio_repr_id)
        .get_result(conn)?;
    Ok(AudioRepresentationId(id))
}

#[instrument(skip(conn), level = "trace")]
pub fn insert_image_representation(
    conn: &mut DbConn,
    repr: &ImageRepresentation,
) -> Result<ImageRepresentationId> {
    use schema::ImageRepresentation;

    assert!(repr.id.0 == 0);

    let id = diesel::insert_into(ImageRepresentation::table)
        .values((
            ImageRepresentation::asset_id.eq(repr.asset_id.0),
            ImageRepresentation::format_name.eq(&repr.format_name),
            ImageRepresentation::width.eq(repr.width),
            ImageRepresentation::height.eq(repr.height),
            ImageRepresentation::file_size.eq(repr.file_size),
            ImageRepresentation::file_key.eq(&repr.file_key),
        ))
        .returning(ImageRepresentation::image_repr_id)
        .get_result(conn)?;
    Ok(ImageRepresentationId(id))
}

#[instrument(skip(conn), level = "trace")]
pub fn get_image_representation(
    conn: &mut DbConn,
    id: ImageRepresentationId,
) -> Result<ImageRepresentation> {
    use schema::ImageRepresentation;

    let db_ir: DbImageRepresentation = ImageRepresentation::table.find(id.0).first(conn)?;
    db_ir.try_into()
}

#[tracing::instrument(skip(conn), level = "trace")]
pub fn get_image_representations(
    conn: &mut DbConn,
    asset_id: AssetId,
) -> Result<Vec<ImageRepresentation>> {
    use schema::ImageRepresentation;

    let db_irs: Vec<DbImageRepresentation> = ImageRepresentation::table
        .filter(ImageRepresentation::asset_id.eq(asset_id.0))
        .load(conn)?;
    db_irs
        .into_iter()
        .map(|db_image_repr| db_image_repr.try_into())
        .collect::<Result<Vec<_>>>()
}

#[tracing::instrument(skip(conn), level = "trace")]
pub fn get_audio_representations(
    conn: &mut DbConn,
    asset_id: AssetId,
) -> Result<Vec<AudioRepresentation>> {
    use schema::AudioRepresentation;
    let db_reprs: Vec<DbAudioRepresentation> = AudioRepresentation::table
        .filter(AudioRepresentation::asset_id.eq(asset_id.0))
        .load(conn)?;
    db_reprs
        .into_iter()
        .map(|db_repr| db_repr.try_into())
        .collect::<Result<Vec<_>>>()
}
