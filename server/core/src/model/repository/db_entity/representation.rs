use diesel::{Queryable, Selectable};

use crate::model::{
    AssetId, AudioRepresentation, AudioRepresentationId, ImageRepresentation,
    ImageRepresentationId, VideoRepresentation, VideoRepresentationId,
};

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::VideoRepresentation)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbVideoRepresentation {
    pub video_repr_id: i64,
    pub asset_id: i64,
    pub codec_name: String,
    pub width: i32,
    pub height: i32,
    pub bitrate: i64,
    pub file_key: String,
    pub media_info_key: String,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::ImageRepresentation)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbImageRepresentation {
    pub image_repr_id: i64,
    pub asset_id: i64,
    pub format_name: String,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
    pub file_key: String,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::AudioRepresentation)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbAudioRepresentation {
    pub audio_repr_id: i64,
    pub asset_id: i64,
    pub codec_name: String,
    pub file_key: String,
    pub media_info_key: String,
}

impl TryFrom<DbImageRepresentation> for ImageRepresentation {
    type Error = eyre::Report;

    fn try_from(value: DbImageRepresentation) -> Result<Self, Self::Error> {
        Ok(ImageRepresentation {
            id: ImageRepresentationId(value.image_repr_id),
            asset_id: AssetId(value.asset_id),
            format_name: value.format_name,
            width: value.width,
            height: value.height,
            file_size: value.file_size,
            file_key: value.file_key,
        })
    }
}

impl TryFrom<DbVideoRepresentation> for VideoRepresentation {
    type Error = eyre::Report;

    fn try_from(value: DbVideoRepresentation) -> Result<Self, Self::Error> {
        Ok(VideoRepresentation {
            id: VideoRepresentationId(value.video_repr_id),
            asset_id: AssetId(value.asset_id),
            codec_name: value.codec_name,
            width: value.width,
            height: value.height,
            bitrate: value.bitrate,
            file_key: value.file_key,
            media_info_key: value.media_info_key,
        })
    }
}
impl TryFrom<DbAudioRepresentation> for AudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: DbAudioRepresentation) -> Result<Self, Self::Error> {
        Ok(AudioRepresentation {
            id: AudioRepresentationId(value.audio_repr_id),
            asset_id: AssetId(value.asset_id),
            codec_name: value.codec_name.clone(),
            file_key: value.file_key.clone(),
            media_info_key: value.media_info_key.clone(),
        })
    }
}
