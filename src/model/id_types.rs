use std::fmt::Display;

use serde::Serialize;

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Copy, Serialize)]
#[sqlx(transparent)]
pub struct AssetRootDirId(pub i64);

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Copy, Serialize)]
#[sqlx(transparent)]
pub struct AssetId(pub i64);

impl From<i64> for AssetRootDirId {
    fn from(value: i64) -> Self {
        AssetRootDirId(value)
    }
}

impl From<i64> for AssetId {
    fn from(value: i64) -> Self {
        AssetId(value)
    }
}

impl Display for AssetRootDirId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("AssetRootDirId({})", self.0))
    }
}

impl Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("AssetId({})", self.0))
    }
}
