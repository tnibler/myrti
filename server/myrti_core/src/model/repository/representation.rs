use diesel::prelude::*;
use eyre::{eyre, Context, Result};
use tracing::instrument;

use crate::model::{
    repository::db_entity::{DbAudioRepresentation, DbImageRepresentation, DbVideoRepresentation},
    AssetId, AudioRepresentation, AudioRepresentationId, CreateVideoRepresentation,
    ImageRepresentation, ImageRepresentationId, VideoRepresentation, VideoRepresentationId,
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
        .filter(
            VideoRepresentation::asset_id
                .eq(asset_id.0)
                .and(VideoRepresentation::created_status.eq(1)),
        )
        .load(conn)?;

    db_video_reprs
        .into_iter()
        .map(|db_vr| db_vr.try_into())
        .collect::<Result<Vec<_>>>()
}

#[instrument(skip(conn), level = "trace")]
pub fn insert_video_representation(
    conn: &mut DbConn,
    repr: CreateVideoRepresentation,
) -> Result<VideoRepresentationId> {
    use schema::VideoRepresentation;

    let id = diesel::insert_into(VideoRepresentation::table)
        .values((
            VideoRepresentation::asset_id.eq(repr.asset_id.0),
            VideoRepresentation::name.eq(repr.name),
            VideoRepresentation::codec_name.eq(repr.codec_name),
            VideoRepresentation::created_status.eq(0),
        ))
        .returning(VideoRepresentation::video_repr_id)
        .get_result(conn)
        .wrap_err("error inserting into table VideoRepresentation")?;
    Ok(VideoRepresentationId(id))
}

#[instrument(skip(conn), level = "trace")]
pub fn finalize_video_representation(conn: &mut DbConn, repr: &VideoRepresentation) -> Result<()> {
    use schema::VideoRepresentation;
    let n_affected = diesel::update(
        VideoRepresentation::table.filter(
            VideoRepresentation::video_repr_id
                .eq(repr.id.0)
                .and(VideoRepresentation::asset_id.eq(repr.asset_id.0))
                .and(VideoRepresentation::created_status.eq(0))
                .and(VideoRepresentation::name.eq(&repr.name))
                .and(VideoRepresentation::codec_name.eq(&repr.codec_name)),
        ),
    )
    .set((
        VideoRepresentation::width.eq(repr.width),
        VideoRepresentation::height.eq(repr.height),
        VideoRepresentation::bitrate.eq(repr.bitrate),
        VideoRepresentation::created_status.eq(1),
    ))
    .execute(conn)
    .context("error updating table VideoRepresentation")?;
    if n_affected == 1 {
        Ok(())
    } else {
        Err(eyre!("did not find VideoRepresentation row to update"))
    }
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
        .wrap_err("error querying for Image Asset representations")
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
