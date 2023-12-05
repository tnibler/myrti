use chrono::NaiveDateTime;
use eyre::eyre;
use serde::{Deserialize, Serialize};

use core::model;

mod album;
mod asset;
mod asset_root_dir;
mod job;
mod timeline;
pub use album::*;
pub use asset::*;
pub use asset_root_dir::*;
pub use job::*;
pub use timeline::*;

macro_rules! impl_api_id {
    ($ident:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
        pub struct $ident(pub String);

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

impl_api_id!(AlbumId);
impl_api_id!(AssetId);
impl_api_id!(AssetRootDirId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetMetadata {
    // exif data doesn't contain timezone info afaik
    pub taken_date: Option<NaiveDateTime>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    #[serde(flatten)]
    pub ty: AssetMetadataType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum AssetMetadataType {
    Video { duration: Option<i32> },
    Image { format: Option<String> },
}
