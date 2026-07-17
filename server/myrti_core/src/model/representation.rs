use crate::model::{ImageAssetId, VideoAssetId};

use super::{AssetId, AudioRepresentationId, ImageRepresentationId, VideoRepresentationId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateVideoRepresentation {
    pub video_asset_id: VideoAssetId,
    pub name: String,
    pub codec_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoRepresentation {
    pub id: VideoRepresentationId,
    pub video_asset_id: VideoAssetId,
    pub name: String,
    pub codec_name: String,
    pub width: i32,
    pub height: i32,
    pub bitrate: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateAudioRepresentation {
    pub video_asset_id: VideoAssetId,
    pub name: String,
    pub codec_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioRepresentation {
    pub id: AudioRepresentationId,
    pub video_asset_id: VideoAssetId,
    pub name: String,
    pub codec_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageRepresentation {
    pub id: ImageRepresentationId,
    pub image_asset_id: ImageAssetId,
    pub format_name: String,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
    pub file_key: String,
}
