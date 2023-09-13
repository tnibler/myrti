use serde::Serialize;
use std::fmt::Display;

macro_rules! impl_id {
    ($ident:ident) => {
        #[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Copy, Hash, Serialize)]
        #[sqlx(transparent)]
        pub struct $ident(pub i64);

        impl From<i64> for $ident {
            fn from(value: i64) -> Self {
                $ident(value)
            }
        }

        impl Display for $ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_fmt(format_args!(concat!(stringify!($ident), "({})"), self.0))
            }
        }
    };
}

impl_id!(AssetId);
impl_id!(AssetRootDirId);
impl_id!(AlbumId);
impl_id!(AlbumEntryId);
impl_id!(DataDirId);
impl_id!(DuplicateAssetId);
impl_id!(VideoRepresentationId);
impl_id!(AudioRepresentationId);
