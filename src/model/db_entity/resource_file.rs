use chrono::NaiveDateTime;

use crate::model::{DataDirId, ResourceFileId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbResourceFile {
    pub id: ResourceFileId,
    pub data_dir_id: DataDirId,
    pub path_in_data_dir: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbResourceFileResolved {
    pub id: ResourceFileId,
    pub data_dir_id: DataDirId,
    pub path_in_data_dir: String,
    pub data_dir_path: String,
    pub created_at: NaiveDateTime,
}
