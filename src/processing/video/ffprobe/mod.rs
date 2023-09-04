mod command;
pub mod streams;
pub mod video_rotation;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FFProbeStreams {
    pub video: VideoStream,
    pub audio: Option<AudioStream>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioStream {
    pub codec_name: String,
    pub sample_rate: i64,
    pub bitrate: i64,
    pub channels: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoStream {
    pub codec_name: String,
    pub width: i64,
    pub height: i64,
    pub bitrate: i64,
    pub rotation: Option<i32>,
}

pub struct FFProbe {}
