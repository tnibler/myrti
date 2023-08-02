use crate::model::{AssetId, AudioRepresentationId, ResourceFileId, VideoRepresentationId};

#[derive(Debug, Clone)]
pub struct DbVideoRepresentation {
    pub id: VideoRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub width: i32,
    pub height: i32,
    pub bitrate: i32,
    pub resource_file_id: ResourceFileId,
}

#[derive(Debug, Clone)]
pub struct DbAudioRepresentation {
    pub id: AudioRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub bitrate: i32,
    pub resource_file_id: ResourceFileId,
}
