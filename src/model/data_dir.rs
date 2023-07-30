use std::path::PathBuf;

use super::{db_entity, util::path_to_string, DataDirId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    pub id: DataDirId,
    pub path: PathBuf,
}

impl TryFrom<&db_entity::DbDataDir> for DataDir {
    type Error = eyre::Report;

    fn try_from(value: &db_entity::DbDataDir) -> Result<Self, Self::Error> {
        Ok(DataDir {
            id: value.id,
            path: PathBuf::from(value.path.clone()),
        })
    }
}

impl TryFrom<db_entity::DbDataDir> for DataDir {
    type Error = eyre::Report;

    fn try_from(value: db_entity::DbDataDir) -> Result<Self, Self::Error> {
        Ok(DataDir {
            id: value.id,
            path: PathBuf::from(value.path),
        })
    }
}

impl TryFrom<&DataDir> for db_entity::DbDataDir {
    type Error = eyre::Report;

    fn try_from(value: &DataDir) -> Result<Self, Self::Error> {
        let path = path_to_string(&value.path)?;
        Ok(db_entity::DbDataDir { id: value.id, path })
    }
}

impl TryFrom<DataDir> for db_entity::DbDataDir {
    type Error = eyre::Report;

    fn try_from(value: DataDir) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
