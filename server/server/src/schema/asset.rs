use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use myrti_core::model;
use std::borrow::Cow;

use crate::{mime_type::guess_mime_type, schema::FileId};

use super::{AssetId, AssetRootDirId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum AssetType {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetFile {
    pub file_id: FileId,
    pub asset_root_id: AssetRootDirId,
    pub path_in_root: String,
    pub width: i32,
    pub height: i32,
    pub added_at: DateTime<Utc>,
    pub rotation_correction: i32,
    pub mirror_correction: MirrorCorrection,
    pub mime_type: String,
    pub thumbhash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub asset_id: AssetId,
    pub taken_date: DateTime<Utc>,

    // #[serde(flatten)]
    pub rep_file: AssetFile,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetWithSpe {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(flatten)]
    pub spe: AssetSpe,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", tag = "assetType")]
pub enum AssetSpe {
    Image(Image),
    Video(Video),
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    #[schema(value_type = Vec::<ImageRepresentation>)]
    pub representations: Box<serde_json::value::RawValue>,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum MirrorCorrection {
    None,
    Horizontal,
    Vertical,
}

impl From<model::MirrorCorrection> for MirrorCorrection {
    fn from(value: model::MirrorCorrection) -> Self {
        match value {
            model::MirrorCorrection::None => MirrorCorrection::None,
            model::MirrorCorrection::Horizontal => MirrorCorrection::Horizontal,
            model::MirrorCorrection::Vertical => MirrorCorrection::Vertical,
        }
    }
}

impl From<MirrorCorrection> for model::MirrorCorrection {
    fn from(value: MirrorCorrection) -> Self {
        match value {
            MirrorCorrection::None => model::MirrorCorrection::None,
            MirrorCorrection::Horizontal => model::MirrorCorrection::Horizontal,
            MirrorCorrection::Vertical => model::MirrorCorrection::Vertical,
        }
    }
}

impl From<model::Asset> for Asset {
    fn from(value: model::Asset) -> Self {
        Asset {
            asset_id: value.base.id.into(),
            taken_date: value.base.taken_date,
            rep_file: value.rep_file.clone().into(),
        }
    }
}

impl From<&model::Asset> for Asset {
    fn from(value: &model::Asset) -> Self {
        value.to_owned().into()
    }
}

impl From<&model::AssetFile> for AssetFile {
    fn from(value: &model::AssetFile) -> Self {
        let mime_type = guess_mime_type(&value.file_type)
            .unwrap_or(match value.ty {
                model::AssetType::Image => Cow::Borrowed("image"),
                model::AssetType::Video => Cow::Borrowed("video"),
            })
            .into_owned();

        AssetFile {
            file_id: value.id.into(),
            asset_root_id: value.root_dir_id.into(),
            path_in_root: value.file_path.to_string(),
            width: value.size.width,
            height: value.size.height,
            added_at: value.added_at,
            mime_type,
            rotation_correction: match value.rotation_correction {
                model::RotationCorrection::CW0 => 0,
                model::RotationCorrection::CW90 => 90,
                model::RotationCorrection::CW180 => 180,
                model::RotationCorrection::CW270 => 270,
            },
            mirror_correction: value.mirror_correction.into(),
            thumbhash: value.thumbhash.clone(),
        }
    }
}

impl From<model::AssetFile> for AssetFile {
    fn from(value: model::AssetFile) -> Self {
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
