use eyre::Result;
use std::path::PathBuf;

use super::{
    db_entity::{DbImageInfo, DbVideoInfo},
    AssetBase, AssetId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Video {
    pub dash_manifest_path: Option<PathBuf>,
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

    fn try_from(_value: &DbImageInfo) -> std::result::Result<Self, Self::Error> {
        Ok(Image {})
    }
}

impl TryFrom<DbImageInfo> for Image {
    type Error = eyre::Report;

    fn try_from(_value: DbImageInfo) -> std::result::Result<Self, Self::Error> {
        Ok(Image {})
    }
}

impl Video {
    pub fn try_to_db_video_info(&self, asset_id: AssetId) -> Result<DbVideoInfo> {
        Ok(DbVideoInfo {
            asset_id,
            dash_manifest_path: self
                .dash_manifest_path
                .clone()
                .map(|p| p.to_str().unwrap().to_string()),
        })
    }
}

impl TryFrom<&DbVideoInfo> for Video {
    type Error = eyre::Report;

    fn try_from(value: &DbVideoInfo) -> std::result::Result<Self, Self::Error> {
        Ok(Video {
            dash_manifest_path: value.dash_manifest_path.as_ref().map(|p| p.into()),
        })
    }
}

impl TryFrom<DbVideoInfo> for Video {
    type Error = eyre::Report;

    fn try_from(value: DbVideoInfo) -> std::result::Result<Self, Self::Error> {
        (&value).try_into()
    }
}
