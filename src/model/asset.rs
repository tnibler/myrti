use eyre::Result;

use super::{
    repository::db_entity::{DbImageInfo, DbVideoInfo},
    AssetBase, AssetId, ResourceFileId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    pub codec_name: String,
    pub dash_resource_dir: Option<ResourceFileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetAll {
    Image(Image),
    Video(Video),
}

pub struct FullAsset {
    pub base: AssetBase,
    pub asset: AssetAll,
}

impl Image {
    pub fn try_to_db_image_info(&self, asset_id: AssetId) -> Result<DbImageInfo> {
        Ok(DbImageInfo { asset_id })
    }
}

impl TryFrom<&DbImageInfo> for Image {
    type Error = eyre::Report;

    fn try_from(_value: &DbImageInfo) -> Result<Self, Self::Error> {
        Ok(Image {})
    }
}

impl TryFrom<DbImageInfo> for Image {
    type Error = eyre::Report;

    fn try_from(_value: DbImageInfo) -> Result<Self, Self::Error> {
        Ok(Image {})
    }
}

impl Video {
    pub fn try_to_db_video_info(&self, asset_id: AssetId) -> Result<DbVideoInfo> {
        Ok(DbVideoInfo {
            asset_id,
            codec_name: self.codec_name.clone(),
            dash_resource_dir: self.dash_resource_dir,
        })
    }
}

impl TryFrom<&DbVideoInfo> for Video {
    type Error = eyre::Report;

    fn try_from(value: &DbVideoInfo) -> Result<Self, Self::Error> {
        Ok(Video {
            codec_name: value.codec_name.clone(),
            dash_resource_dir: value.dash_resource_dir,
        })
    }
}

impl TryFrom<DbVideoInfo> for Video {
    type Error = eyre::Report;

    fn try_from(value: DbVideoInfo) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
