use camino::Utf8PathBuf as PathBuf;
use serde::Serialize;

use super::{AssetId, AssetType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetThumbnails {
    pub id: AssetId,
    pub ty: AssetType,
    pub thumb_small_square_avif: bool,
    pub thumb_small_square_webp: bool,
    pub thumb_large_orig_avif: bool,
    pub thumb_large_orig_webp: bool,
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
