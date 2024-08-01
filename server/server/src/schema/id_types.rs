use eyre::eyre;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use core::model;

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

// The actual struct type declaration is not part of the macro so that utoipa_discover
// picks up the declaration and notices the derive(ToSchema) on it.
// That doesn't work if the declaration is in a macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct AssetId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct AlbumId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct AlbumItemId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct AssetRootDirId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct ImageRepresentationId(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, ToSchema)]
pub struct TimelineGroupId(pub String);

impl_api_id!(AlbumId);
impl_api_id!(AlbumItemId);
impl_api_id!(AssetId);
impl_api_id!(AssetRootDirId);
impl_api_id!(ImageRepresentationId);
impl_api_id!(TimelineGroupId);
