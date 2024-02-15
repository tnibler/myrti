use camino::Utf8PathBuf as PathBuf;
use serde::Serialize;

use super::AssetId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetThumbnails {
    pub id: AssetId,
    pub has_thumb_large_orig: bool,
    pub has_thumb_small_square: bool,
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
