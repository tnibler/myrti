use super::{
    AssetId, AudioRepresentationId, ImageRepresentationId, VideoRepresentationId,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoRepresentation {
    pub id: VideoRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub width: i32,
    pub height: i32,
    pub bitrate: i64,
    pub file_key: String,
    pub media_info_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioRepresentation {
    pub id: AudioRepresentationId,
    pub asset_id: AssetId,
    pub codec_name: String,
    pub file_key: String,
    pub media_info_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageRepresentation {
    pub id: ImageRepresentationId,
    pub asset_id: AssetId,
    pub format_name: String,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
    pub file_key: String,
}
