use eyre::{eyre, Report};

use super::{repository::db_entity::DbAsset, AssetBase, ResourceFileId};

#[derive(Debug, Clone)]
pub struct Image {}

#[derive(Debug, Clone)]
pub struct Video {
    pub codec_name: String,
    pub dash_resource_dir: Option<ResourceFileId>,
}

#[derive(Debug, Clone)]
pub enum AssetSpe {
    Image(Image),
    Video(Video),
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub base: AssetBase,
    pub sp: AssetSpe,
}

#[derive(Debug, Clone)]
pub struct VideoAsset {
    pub base: AssetBase,
    pub video: Video,
}

#[derive(Debug, Clone)]
pub struct ImageAsset {
    pub base: AssetBase,
    pub image: Image,
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
