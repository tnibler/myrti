use crate::model::{DataDirId, ResourceFileId};
use chrono::{DateTime, Utc};
use eyre::{eyre, Report};
use std::path::PathBuf;

use super::{db_entity, util::path_to_string};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceFile {
    pub id: ResourceFileId,
    pub data_dir_id: DataDirId,
    pub path_in_data_dir: PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceFileResolved {
    pub id: ResourceFileId,
    pub data_dir_id: DataDirId,
    pub path_in_data_dir: PathBuf,
    pub data_dir_path: PathBuf,
    pub path_on_disk: PathBuf,
    pub created_at: DateTime<Utc>,
}

impl ResourceFileResolved {
    pub fn path_on_disk(&self) -> PathBuf {
        self.data_dir_path.join(&self.path_in_data_dir)
    }
}

impl TryFrom<&db_entity::DbResourceFile> for ResourceFile {
    type Error = eyre::Report;

    fn try_from(value: &db_entity::DbResourceFile) -> Result<Self, Self::Error> {
        Ok(ResourceFile {
            id: value.id,
            data_dir_id: value.data_dir_id,
            path_in_data_dir: PathBuf::from(&value.path_in_data_dir),
            created_at: value.created_at.and_utc(),
        })
    }
}

impl TryFrom<&db_entity::DbResourceFileResolved> for ResourceFileResolved {
    type Error = eyre::Report;

    fn try_from(value: &db_entity::DbResourceFileResolved) -> Result<Self, Self::Error> {
        let path_in_data_dir = PathBuf::from(&value.path_in_data_dir);
        let data_dir_path = PathBuf::from(&value.data_dir_path);
        let path_on_disk = data_dir_path.join(&path_in_data_dir);
        Ok(ResourceFileResolved {
            id: value.id,
            data_dir_id: value.data_dir_id,
            path_in_data_dir,
            data_dir_path,
            path_on_disk,
            created_at: value.created_at.and_utc(),
        })
    }
}

impl TryFrom<db_entity::DbResourceFileResolved> for ResourceFileResolved {
    type Error = eyre::Report;

    fn try_from(value: db_entity::DbResourceFileResolved) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl TryFrom<ResourceFile> for db_entity::DbResourceFile {
    type Error = eyre::Report;

    fn try_from(value: ResourceFile) -> Result<Self, Self::Error> {
        let path_in_data_dir = path_to_string(value.path_in_data_dir)?;
        Ok(db_entity::DbResourceFile {
            id: value.id,
            data_dir_id: value.data_dir_id,
            path_in_data_dir,
            created_at: value.created_at.naive_utc(),
        })
    }
}

impl TryFrom<&ResourceFileResolved> for db_entity::DbResourceFile {
    type Error = eyre::Report;

    fn try_from(value: &ResourceFileResolved) -> Result<Self, Self::Error> {
        let path_in_data_dir = path_to_string(&value.path_in_data_dir)?;
        Ok(db_entity::DbResourceFile {
            id: value.id,
            data_dir_id: value.data_dir_id,
            path_in_data_dir,
            created_at: value.created_at.naive_utc(),
        })
    }
}

impl TryFrom<ResourceFileResolved> for db_entity::DbResourceFile {
    type Error = eyre::Report;

    fn try_from(value: ResourceFileResolved) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}

impl From<ResourceFileResolved> for ResourceFile {
    fn from(value: ResourceFileResolved) -> Self {
        (&value).into()
    }
}

impl From<&ResourceFileResolved> for ResourceFile {
    fn from(value: &ResourceFileResolved) -> Self {
        ResourceFile {
            id: value.id,
            data_dir_id: value.data_dir_id,
            path_in_data_dir: value.path_in_data_dir.clone(),
            created_at: value.created_at,
        }
    }
}
