use std::default;

use camino::Utf8PathBuf as PathBuf;
use chrono::{DateTime, Utc};
use eyre::{Report, eyre};

use super::{AssetBase, AssetFile, AssetRootDirId, FileId, GpsCoordinates, Size, TimestampInfo};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Image {
    pub file_id: FileId,
    pub image_format_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Video {
    pub file_id: FileId,
    pub video_codec_name: String,
    pub video_bitrate: Option<i64>,
    pub audio_codec_name: Option<String>,
    pub is_original_streamable: bool,
    pub max_iframe_interval: Option<i32>,
    pub frame_rate: Option<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetSpe {
    Image(Image),
    Video(Video),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Asset {
    pub base: AssetBase,
    pub rep_file: AssetFile,
    pub sp: AssetSpe,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoAsset {
    pub base: AssetBase,
    pub rep_file: AssetFile,
    pub video: Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageAsset {
    pub base: AssetBase,
    pub rep_file: AssetFile,
    pub image: Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateAsset {
    pub base: CreateAssetBase,
    pub spe: CreateAssetSpe,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CreateAssetSpe {
    Image(CreateAssetImage),
    Video(CreateAssetVideo),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub enum RotationCorrection {
    #[default]
    CW0,
    CW90,
    CW180,
    CW270,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub enum MirrorCorrection {
    #[default]
    None,
    Horizontal,
    Vertical,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum HasGhiIndex {
    None,
    VideoOnly,
    AudioOnly,
    VideoAudio,
}

impl HasGhiIndex {
    pub const fn includes_video(&self) -> bool {
        match self {
            Self::VideoOnly | Self::VideoAudio => true,
            Self::AudioOnly | Self::None => false,
        }
    }
    pub const fn includes_audio(&self) -> bool {
        match self {
            Self::AudioOnly | Self::VideoAudio => true,
            Self::VideoOnly | Self::None => false,
        }
    }
}

impl TryFrom<i32> for HasGhiIndex {
    type Error = eyre::Report;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::VideoOnly),
            2 => Ok(Self::AudioOnly),
            3 => Ok(Self::VideoAudio),
            other => Err(eyre!("invalid HasGhiIndex value {}", other)),
        }
    }
}

impl From<HasGhiIndex> for i32 {
    fn from(value: HasGhiIndex) -> Self {
        match value {
            HasGhiIndex::None => 0,
            HasGhiIndex::VideoOnly => 1,
            HasGhiIndex::AudioOnly => 2,
            HasGhiIndex::VideoAudio => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateAssetBase {
    pub root_dir_id: AssetRootDirId,
    pub file_type: String,
    pub file_path: PathBuf,
    pub taken_date: DateTime<Utc>,
    pub timestamp_info: TimestampInfo,
    pub size: Size,
    pub is_hidden: bool,
    pub rotation_correction: RotationCorrection,
    /// Seahash of the file, if already computed
    pub hash: Option<u64>,
    /// JSON output of exiftool
    pub exiftool_output: Vec<u8>,
    pub gps_coordinates: Option<GpsCoordinates>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateAssetImage {
    pub image_format_name: String,
}

#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub struct FFProbeOutput(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateAssetVideo {
    pub ffprobe_output: FFProbeOutput,
    pub video_codec_name: String,
    pub video_bitrate: Option<i64>,
    pub video_duration_ms: Option<i64>,
    pub audio_codec_name: Option<String>,
    pub is_original_streamable: bool,
    pub max_iframe_interval: Option<i32>,
    pub frame_rate: Option<(i32, i32)>,
}

impl From<&ImageAsset> for Asset {
    fn from(value: &ImageAsset) -> Self {
        Asset {
            base: value.base.clone(),
            rep_file: value.rep_file.clone(),
            sp: AssetSpe::Image(value.image.clone()),
        }
    }
}

impl From<&VideoAsset> for Asset {
    fn from(value: &VideoAsset) -> Self {
        Asset {
            base: value.base.clone(),
            rep_file: value.rep_file.clone(),
            sp: AssetSpe::Video(value.video.clone()),
        }
    }
}

impl From<ImageAsset> for Asset {
    fn from(value: ImageAsset) -> Self {
        (&value).into()
    }
}

impl From<VideoAsset> for Asset {
    fn from(value: VideoAsset) -> Self {
        (&value).into()
    }
}

impl TryFrom<&Asset> for VideoAsset {
    type Error = Report;

    fn try_from(value: &Asset) -> std::result::Result<Self, Self::Error> {
        match &value.sp {
            AssetSpe::Image(_) => Err(eyre!("not a video")),
            AssetSpe::Video(video) => Ok(VideoAsset {
                base: value.base.clone(),
                rep_file: value.rep_file.clone(),
                video: video.clone(),
            }),
        }
    }
}

impl TryFrom<Asset> for VideoAsset {
    type Error = Report;

    fn try_from(value: Asset) -> std::result::Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&Asset> for ImageAsset {
    type Error = Report;

    fn try_from(value: &Asset) -> std::result::Result<Self, Self::Error> {
        match &value.sp {
            AssetSpe::Image(image) => Ok(ImageAsset {
                base: value.base.clone(),
                rep_file: value.rep_file.clone(),
                image: image.clone(),
            }),
            AssetSpe::Video(_) => Err(eyre!("not an image")),
        }
    }
}

impl TryFrom<Asset> for ImageAsset {
    type Error = Report;

    fn try_from(value: Asset) -> std::result::Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl From<Vec<u8>> for FFProbeOutput {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl std::fmt::Debug for FFProbeOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}B)", self.0.len())
    }
}
