use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use core::model;
use std::borrow::Cow;

use crate::mime_type::guess_mime_type;

use super::{AssetId, AssetMetadata, AssetRootDirId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum AssetType {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: AssetId,
    pub asset_root_id: AssetRootDirId,
    pub path_in_root: String,
    #[serde(rename = "type")]
    pub ty: AssetType,
    pub width: i32,
    pub height: i32,
    pub added_at: DateTime<Utc>,
    pub taken_date: DateTime<Utc>,
    pub metadata: Option<AssetMetadata>,
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetWithSpe {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(flatten)]
    pub spe: AssetSpe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(untagged)]
pub enum AssetSpe {
    Image(Image),
    Video(Video),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub representations: Vec<ImageRepresentation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageRepresentation {
    pub id: String,
    pub format: String,
    pub width: i32,
    pub height: i32,
    pub size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub has_dash: bool,
}

impl From<&model::Asset> for AssetWithSpe {
    fn from(value: &model::Asset) -> Self {
        AssetWithSpe {
            asset: value.into(),
            spe: match &value.sp {
                model::AssetSpe::Image(_image) => AssetSpe::Image(Image {
                    representations: Default::default(), /* FIXME */
                }),
                model::AssetSpe::Video(video) => AssetSpe::Video(Video {
                    has_dash: video.has_dash,
                }),
            },
        }
    }
}

impl From<model::Asset> for AssetWithSpe {
    fn from(value: model::Asset) -> Self {
        (&value).into()
    }
}

impl From<&model::Asset> for Asset {
    fn from(value: &model::Asset) -> Self {
        let mime_type = guess_mime_type(&value.base.file_type)
            .unwrap_or(match value.base.ty {
                model::AssetType::Image => Cow::Borrowed("image"),
                model::AssetType::Video => Cow::Borrowed("video"),
            })
            .into_owned();

        Asset {
            id: value.base.id.into(),
            asset_root_id: value.base.root_dir_id.into(),
            path_in_root: value.base.file_path.to_string(),
            ty: value.base.ty.into(),
            width: value.base.size.width as i32,
            height: value.base.size.height as i32,
            added_at: value.base.added_at,
            taken_date: value.base.taken_date,
            metadata: None,
            mime_type,
        }
    }
}

impl From<model::Asset> for Asset {
    fn from(value: model::Asset) -> Self {
        (&value).into()
    }
}

impl From<AssetType> for model::AssetType {
    fn from(value: AssetType) -> Self {
        match value {
            AssetType::Image => model::AssetType::Image,
            AssetType::Video => model::AssetType::Video,
        }
    }
}

impl From<model::AssetType> for AssetType {
    fn from(value: model::AssetType) -> Self {
        match value {
            model::AssetType::Image => AssetType::Image,
            model::AssetType::Video => AssetType::Video,
        }
    }
}
