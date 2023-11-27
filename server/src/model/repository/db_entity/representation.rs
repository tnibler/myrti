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
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub bitrate: Option<i64>,
    pub file_key: Option<String>,
    pub media_info_key: Option<String>,
    pub is_preallocated_dummy: i64,
}

#[derive(Debug, Clone)]
pub struct DbImageRepresentation {
    pub id: ImageRepresentationId,
    pub asset_id: AssetId,
    pub format_name: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub file_size: Option<i64>,
    pub file_key: Option<String>,
    pub is_preallocated_dummy: i64,
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
        if value.is_preallocated_dummy != 0 {
            return Err(eyre!("ImageRepresentation is preallocated dummy row"));
        }
        Ok(ImageRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            format_name: value.format_name.clone(),
            width: value.width.ok_or(eyre!("columns must be non-null"))?,
            height: value.height.ok_or(eyre!("columns must be non-null"))?,
            file_size: value.file_size.ok_or(eyre!("columns must be non-null"))?,
            file_key: value
                .file_key
                .as_ref()
                .ok_or(eyre!("columns must be non-null"))?
                .clone(),
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
        if value.is_preallocated_dummy != 0 {
            return Err(eyre!("VideoRepresentation is preallocated dummy row"));
        }
        Ok(VideoRepresentation {
            id: value.id,
            asset_id: value.asset_id,
            codec_name: value.codec_name.clone(),
            width: value.width.ok_or(eyre!("columns must be non-null"))?,
            height: value.height.ok_or(eyre!("columns must be non-null"))?,
            bitrate: value.bitrate.ok_or(eyre!("columns must be non-null"))?,
            file_key: value
                .file_key
                .as_ref()
                .ok_or(eyre!("columns must be non-null"))?
                .clone(),
            media_info_key: value
                .media_info_key
                .as_ref()
                .ok_or(eyre!("columns must be non-null"))?
                .clone(),
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
            is_preallocated_dummy: 0,
            format_name: value.format_name.clone(),
            width: Some(value.width),
            height: Some(value.height),
            file_size: Some(value.file_size.clone()),
            file_key: Some(value.file_key.clone()),
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
            is_preallocated_dummy: 0,
            codec_name: value.codec_name.clone(),
            width: Some(value.width),
            height: Some(value.height),
            bitrate: Some(value.bitrate),
            file_key: Some(value.file_key.clone()),
            media_info_key: Some(value.media_info_key.clone()),
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
