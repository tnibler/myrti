use std::path::PathBuf;

use super::{
    db_entity::{DbAssetPathOnDisk, DbAssetThumbnails},
    AssetId, AssetType, ResourceFileId,
};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetThumbnails {
    pub id: AssetId,
    pub ty: AssetType,
    pub thumb_small_square_jpg: Option<ResourceFileId>,
    pub thumb_small_square_webp: Option<ResourceFileId>,
    pub thumb_large_orig_jpg: Option<ResourceFileId>,
    pub thumb_large_orig_webp: Option<ResourceFileId>,
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
            thumb_small_square_jpg: value.thumb_small_square_jpg,
            thumb_small_square_webp: value.thumb_small_square_webp,
            thumb_large_orig_jpg: value.thumb_large_orig_jpg,
            thumb_large_orig_webp: value.thumb_large_orig_webp,
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
            thumb_small_square_jpg: value.thumb_small_square_jpg,
            thumb_small_square_webp: value.thumb_small_square_webp,
            thumb_large_orig_jpg: value.thumb_large_orig_jpg,
            thumb_large_orig_webp: value.thumb_large_orig_webp,
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
