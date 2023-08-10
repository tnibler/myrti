use crate::model::{AssetId, AudioRepresentationId, ResourceFileId, VideoRepresentationId};

#[derive(Debug, Clone)]
pub struct DbVideoRepresentation {
    pub id: VideoRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub width: i64,
    pub height: i64,
    pub bitrate: i32,
    pub path_in_resource_dir: String,
}

#[derive(Debug, Clone)]
pub struct DbAudioRepresentation {
    pub id: AudioRepresentationId,
    pub asset_id: AssetId,
    pub path_in_resource_dir: String,
}
