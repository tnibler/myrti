use eyre::eyre;

use crate::model::{
    AssetId, AudioRepresentation, AudioRepresentationId, ImageRepresentation,
    ImageRepresentationId, VideoRepresentation, VideoRepresentationId,
};

#[derive(Debug, Clone)]
pub struct DbVideoRepresentation {
    pub id: VideoRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub width: i64,
    pub height: i64,
    pub bitrate: i64,
    pub file_key: String,
    pub media_info_key: String,
}

#[derive(Debug, Clone)]
pub struct DbImageRepresentation {
    pub id: ImageRepresentationId,
    pub asset_id: AssetId,
    pub format_name: String,
    pub width: i64,
    pub height: i64,
    pub file_size: i64,
    pub file_key: String,
}

#[derive(Debug, Clone)]
pub struct DbAudioRepresentation {
    pub id: AudioRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub file_key: String,
    pub media_info_key: String,
}

impl TryFrom<&DbImageRepresentation> for ImageRepresentation {
    type Error = eyre::Report;

    fn try_from(value: &DbImageRepresentation) -> Result<Self, Self::Error> {
        Ok(ImageRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            format_name: value.format_name.clone(),
            width: value.width,
            height: value.height,
            file_size: value.file_size,
            file_key: value.file_key.clone(),
        })
    }
}

impl TryFrom<DbImageRepresentation> for ImageRepresentation {
    type Error = eyre::Report;

    fn try_from(value: DbImageRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&DbVideoRepresentation> for VideoRepresentation {
    type Error = eyre::Report;

    fn try_from(value: &DbVideoRepresentation) -> Result<Self, Self::Error> {
        Ok(VideoRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            codec_name: value.codec_name.clone(),
            width: value.width,
            height: value.height,
            bitrate: value.bitrate,
            file_key: value.file_key.clone(),
            media_info_key: value.media_info_key.clone(),
        })
    }
}

impl TryFrom<DbVideoRepresentation> for VideoRepresentation {
    type Error = eyre::Report;

    fn try_from(value: DbVideoRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&DbAudioRepresentation> for AudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: &DbAudioRepresentation) -> Result<Self, Self::Error> {
        Ok(AudioRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            codec_name: value.codec_name.clone(),
            file_key: value.file_key.clone(),
            media_info_key: value.media_info_key.clone(),
        })
    }
}

impl TryFrom<DbAudioRepresentation> for AudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: DbAudioRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&ImageRepresentation> for DbImageRepresentation {
    type Error = eyre::Report;

    fn try_from(value: &ImageRepresentation) -> Result<Self, Self::Error> {
        Ok(DbImageRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            format_name: value.format_name.clone(),
            width: value.width,
            height: value.height,
            file_size: value.file_size.clone(),
            file_key: value.file_key.clone(),
        })
    }
}

impl TryFrom<ImageRepresentation> for DbImageRepresentation {
    type Error = eyre::Report;

    fn try_from(value: ImageRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&VideoRepresentation> for DbVideoRepresentation {
    type Error = eyre::Report;

    fn try_from(value: &VideoRepresentation) -> Result<Self, Self::Error> {
        Ok(DbVideoRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            codec_name: value.codec_name.clone(),
            width: value.width,
            height: value.height,
            bitrate: value.bitrate,
            file_key: value.file_key.clone(),
            media_info_key: value.media_info_key.clone(),
        })
    }
}

impl TryFrom<VideoRepresentation> for DbVideoRepresentation {
    type Error = eyre::Report;

    fn try_from(value: VideoRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&AudioRepresentation> for DbAudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: &AudioRepresentation) -> Result<Self, Self::Error> {
        Ok(DbAudioRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            codec_name: value.codec_name.clone(),
            file_key: value.file_key.clone(),
            media_info_key: value.media_info_key.clone(),
        })
    }
}

impl TryFrom<AudioRepresentation> for DbAudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: AudioRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
