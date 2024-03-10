use chrono::NaiveDateTime;
use eyre::eyre;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use core::model;

pub mod album;
pub mod asset;
mod asset_root_dir;
pub mod timeline;
pub use album::*;
pub use asset_root_dir::*;

macro_rules! impl_api_id {
    ($ident:ident) => {
        impl From<&model::$ident> for $ident {
            fn from(value: &model::$ident) -> Self {
                $ident(value.0.to_string())
            }
        }

        impl From<model::$ident> for $ident {
            fn from(value: model::$ident) -> Self {
                (&value).into()
            }
        }

        impl TryFrom<&$ident> for model::$ident {
            type Error = eyre::Report;
            fn try_from(value: &$ident) -> Result<Self, Self::Error> {
                match value.0.parse::<i64>() {
                    Ok(id) => Ok(model::$ident(id)),
                    Err(_) => Err(eyre!(
                        concat!("Invalid ", stringify!($ident), " {}"),
                        value.0
                    )),
                }
            }
        }

        impl TryFrom<$ident> for model::$ident {
            type Error = eyre::Report;
            fn try_from(value: $ident) -> Result<Self, Self::Error> {
                (&value).try_into()
            }
        }
    };
}

// The actual struct type declaration is not part of the macro so that utoipauto
// picks up the declaration and notices the derive(ToSchema) on it.
// That doesn't work if the declaration is in a macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct AssetId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct AlbumId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct AssetRootDirId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct TimelineGroupId(pub String);

impl_api_id!(AlbumId);
impl_api_id!(AssetId);
impl_api_id!(AssetRootDirId);
impl_api_id!(TimelineGroupId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AssetMetadata {
    // exif data doesn't contain timezone info afaik
    pub taken_date: Option<NaiveDateTime>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    #[serde(flatten)]
    pub ty: AssetMetadataType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub enum AssetMetadataType {
    Video { duration: Option<i32> },
    Image { format: Option<String> },
}
