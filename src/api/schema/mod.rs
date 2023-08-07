use chrono::NaiveDateTime;
use serde::Serialize;

mod asset;
mod asset_root_dir;
mod job;
mod timeline;
pub use asset::*;
pub use asset_root_dir::*;
pub use job::*;
pub use timeline::*;

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
