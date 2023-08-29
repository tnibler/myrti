use crate::model::{AssetId, AudioRepresentationId, VideoRepresentationId};

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
pub struct DbAudioRepresentation {
    pub id: AudioRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub file_key: String,
    pub media_info_key: String,
}
