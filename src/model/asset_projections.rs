use std::path::PathBuf;

use super::{
    repository::db_entity::{DbAssetPathOnDisk, DbAssetThumbnails},
    util::{opt_path_to_string, path_to_string},
    AssetId, AssetType,
};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetThumbnails {
    pub id: AssetId,
    pub ty: AssetType,
    pub thumb_small_square_avif: Option<PathBuf>,
    pub thumb_small_square_webp: Option<PathBuf>,
    pub thumb_large_orig_avif: Option<PathBuf>,
    pub thumb_large_orig_webp: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetPathOnDisk {
    pub id: AssetId,
    pub path_in_asset_root: PathBuf,
    pub asset_root_path: PathBuf,
}

impl AssetPathOnDisk {
    pub fn path_on_disk(&self) -> PathBuf {
        self.asset_root_path.join(&self.path_in_asset_root)
    }
}

impl TryFrom<&AssetThumbnails> for DbAssetThumbnails {
    type Error = eyre::Report;

    fn try_from(value: &AssetThumbnails) -> Result<Self, Self::Error> {
        Ok(DbAssetThumbnails {
            id: value.id,
            ty: value.ty.into(),
            thumb_small_square_avif: opt_path_to_string(&value.thumb_small_square_avif)?,
            thumb_small_square_webp: opt_path_to_string(&value.thumb_small_square_webp)?,
            thumb_large_orig_avif: opt_path_to_string(&value.thumb_large_orig_avif)?,
            thumb_large_orig_webp: opt_path_to_string(&value.thumb_large_orig_webp)?,
        })
    }
}

impl TryFrom<AssetThumbnails> for DbAssetThumbnails {
    type Error = eyre::Report;

    fn try_from(value: AssetThumbnails) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<&DbAssetThumbnails> for AssetThumbnails {
    type Error = eyre::Report;

    fn try_from(value: &DbAssetThumbnails) -> Result<Self, Self::Error> {
        Ok(AssetThumbnails {
            id: value.id,
            ty: value.ty.into(),
            thumb_small_square_avif: value.thumb_small_square_avif.as_ref().map(|p| p.into()),
            thumb_small_square_webp: value.thumb_small_square_webp.as_ref().map(|p| p.into()),
            thumb_large_orig_avif: value.thumb_large_orig_avif.as_ref().map(|p| p.into()),
            thumb_large_orig_webp: value.thumb_large_orig_webp.as_ref().map(|p| p.into()),
        })
    }
}

impl TryFrom<DbAssetThumbnails> for AssetThumbnails {
    type Error = eyre::Report;

    fn try_from(value: DbAssetThumbnails) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<DbAssetPathOnDisk> for AssetPathOnDisk {
    type Error = eyre::Report;

    fn try_from(value: DbAssetPathOnDisk) -> Result<Self, Self::Error> {
        Ok(AssetPathOnDisk {
            id: value.id,
            path_in_asset_root: PathBuf::from(value.path_in_asset_root),
            asset_root_path: PathBuf::from(value.asset_root_path),
        })
    }
}
