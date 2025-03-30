use diesel::prelude::*;
use eyre::{Context, Result};
use tracing::instrument;

use super::db::DbConn;
use super::schema;

#[instrument(skip(conn), level = "debug")]
pub fn insert_acceptable_video_codec(conn: &mut DbConn, codec_name: &str) -> Result<()> {
    use schema::AcceptableVideoCodec;
    diesel::insert_into(AcceptableVideoCodec::table)
        .values(AcceptableVideoCodec::codec_name.eq(codec_name))
        .execute(conn)?;
    Ok(())
}

#[instrument(skip(conn), level = "debug")]
pub fn insert_acceptable_audio_codec(conn: &mut DbConn, codec_name: &str) -> Result<()> {
    use schema::AcceptableAudioCodec;
    diesel::insert_into(AcceptableAudioCodec::table)
        .values(AcceptableAudioCodec::codec_name.eq(codec_name))
        .execute(conn)?;
    Ok(())
}

pub fn set_acceptable_video_codecs(
    conn: &mut DbConn,
    codec_names: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<()> {
    use schema::AcceptableVideoCodec;
    conn.transaction(|conn| {
        diesel::delete(AcceptableVideoCodec::table).execute(conn)?;
        for codec_name in codec_names {
            diesel::insert_into(AcceptableVideoCodec::table)
                .values(AcceptableVideoCodec::codec_name.eq(codec_name.as_ref()))
                .execute(conn)?;
        }
        Ok::<_, eyre::Report>(())
    })
    .wrap_err("Error clearing and setting table AcceptableVideoCodec")?;
    Ok(())
}

pub fn set_acceptable_audio_codecs(
    conn: &mut DbConn,
    codec_names: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<()> {
    use schema::AcceptableAudioCodec;
    conn.transaction(|conn| {
        diesel::delete(AcceptableAudioCodec::table).execute(conn)?;
        for codec_name in codec_names {
            diesel::insert_into(AcceptableAudioCodec::table)
                .values(AcceptableAudioCodec::codec_name.eq(codec_name.as_ref()))
                .execute(conn)?;
        }
        Ok::<_, eyre::Report>(())
    })?;
    Ok(())
}
