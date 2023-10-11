use camino::Utf8PathBuf as PathBuf;
use chrono::{DateTime, Utc};
use eyre::{eyre, Report};

use super::{
    repository::db_entity::DbAsset, AssetBase, AssetRootDirId, AssetType, GpsCoordinates, Size,
    TimestampInfo,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Image {
    pub image_format_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Video {
    pub video_codec_name: String,
    pub video_bitrate: i64,
    pub audio_codec_name: Option<String>,
    pub has_dash: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetSpe {
    Image(Image),
    Video(Video),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Asset {
    pub base: AssetBase,
    pub sp: AssetSpe,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoAsset {
    pub base: AssetBase,
    pub video: Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageAsset {
    pub base: AssetBase,
    pub image: Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CreateAsset {
    pub ty: AssetType,
    pub root_dir_id: AssetRootDirId,
    pub file_type: String,
    pub file_path: PathBuf,
    pub taken_date: DateTime<Utc>,
    pub timestamp_info: TimestampInfo,
    pub size: Size,
    /// degrees clockwise
    pub rotation_correction: Option<i32>,
    /// Seahash of the file, if already computed
    pub hash: Option<u64>,
    pub sp: AssetSpe,
    pub gps_coordinates: Option<GpsCoordinates>,
}

impl From<&ImageAsset> for Asset {
    fn from(value: &ImageAsset) -> Self {
        Asset {
            base: value.base.clone(),
            sp: AssetSpe::Image(value.image.clone()),
        }
    }
}

impl From<&VideoAsset> for Asset {
    fn from(value: &VideoAsset) -> Self {
        Asset {
            base: value.base.clone(),
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

impl TryFrom<&DbAsset> for VideoAsset {
    type Error = Report;

    fn try_from(value: &DbAsset) -> Result<Self, Self::Error> {
        Asset::try_from(value)?.try_into()
    }
}

impl TryFrom<DbAsset> for VideoAsset {
    type Error = Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        Asset::try_from(&value)?.try_into()
    }
}

impl TryFrom<&DbAsset> for ImageAsset {
    type Error = Report;

    fn try_from(value: &DbAsset) -> Result<Self, Self::Error> {
        Asset::try_from(value)?.try_into()
    }
}

impl TryFrom<DbAsset> for ImageAsset {
    type Error = Report;

    fn try_from(value: DbAsset) -> Result<Self, Self::Error> {
        Asset::try_from(&value)?.try_into()
    }
}
