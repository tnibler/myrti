use std::path::PathBuf;

use super::{
    repository::db_entity::{DbAudioRepresentation, DbVideoRepresentation},
    util::path_to_string,
    AssetId, AudioRepresentationId, VideoRepresentationId,
};

#[derive(Debug, Clone)]
pub struct VideoRepresentation {
    pub id: VideoRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub width: i64,
    pub height: i64,
    pub bitrate: i64,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AudioRepresentation {
    pub id: AudioRepresentationId,
    pub asset_id: AssetId,
    pub path: PathBuf,
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
            path: PathBuf::from(&value.path),
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
            path: PathBuf::from(&value.path),
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
            path: path_to_string(&value.path)?,
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
            path: path_to_string(&value.path)?,
        })
    }
}

impl TryFrom<AudioRepresentation> for DbAudioRepresentation {
    type Error = eyre::Report;

    fn try_from(value: AudioRepresentation) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
