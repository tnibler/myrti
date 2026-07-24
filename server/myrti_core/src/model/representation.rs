use super::{AudioRepresentationId, FileId, ImageRepresentationId, VideoRepresentationId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateVideoRepresentation {
    pub file_id: FileId,
    pub name: String,
    pub codec_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoRepresentation {
    pub id: VideoRepresentationId,
    pub file_id: FileId,
    pub name: String,
    pub codec_name: String,
    pub width: i32,
    pub height: i32,
    pub bitrate: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateAudioRepresentation {
    pub file_id: FileId,
    pub name: String,
    pub codec_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioRepresentation {
    pub id: AudioRepresentationId,
    pub file_id: FileId,
    pub name: String,
    pub codec_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageRepresentation {
    pub id: ImageRepresentationId,
    pub file_id: FileId,
    pub format_name: String,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
    pub file_key: String,
}
