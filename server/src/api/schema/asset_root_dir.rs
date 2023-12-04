use camino::Utf8PathBuf as PathBuf;
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::AssetRootDirId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRoot {
    pub id: AssetRootDirId,
    pub path: PathBuf,
    pub num_assets: i32,
    pub last_full_reindex: Option<DateTime<Utc>>,
    pub last_change: Option<DateTime<Utc>>,
}
