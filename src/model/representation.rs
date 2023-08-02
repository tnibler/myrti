use super::{
    db_entity::{DbAudioRepresentation, DbVideoRepresentation},
    AssetId, AudioRepresentationId, ResourceFileId, VideoRepresentationId,
};

#[derive(Debug, Clone)]
pub struct VideoRepresentation {
    pub id: VideoRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub width: i32,
    pub height: i32,
    pub bitrate: i32,
    pub resource_file_id: ResourceFileId,
}

#[derive(Debug, Clone)]
pub struct AudioRepresentation {
    pub id: AudioRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub bitrate: i32,
    pub resource_file_id: ResourceFileId,
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
            resource_file_id: value.resource_file_id,
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
            bitrate: value.bitrate,
            resource_file_id: value.resource_file_id,
        })
    }
}

impl TryFrom<DbAudioRepresentation> for AudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: DbAudioRepresentation) -> Result<Self, Self::Error> {
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
            resource_file_id: value.resource_file_id,
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
            bitrate: value.bitrate,
            resource_file_id: value.resource_file_id,
        })
    }
}

impl TryFrom<AudioRepresentation> for DbAudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: AudioRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
