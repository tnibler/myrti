use camino::Utf8PathBuf as PathBuf;

use super::{repository::db_entity::DbDataDir, util::path_to_string, DataDirId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    pub id: DataDirId,
    pub path: PathBuf,
}

impl TryFrom<&DbDataDir> for DataDir {
    type Error = eyre::Report;

    fn try_from(value: &DbDataDir) -> Result<Self, Self::Error> {
        Ok(DataDir {
            id: value.id,
            path: PathBuf::from(value.path.clone()),
        })
    }
}

impl TryFrom<DbDataDir> for DataDir {
    type Error = eyre::Report;

    fn try_from(value: DbDataDir) -> Result<Self, Self::Error> {
        Ok(DataDir {
            id: value.id,
            path: PathBuf::from(value.path),
        })
    }
}

impl TryFrom<&DataDir> for DbDataDir {
    type Error = eyre::Report;

    fn try_from(value: &DataDir) -> Result<Self, Self::Error> {
        let path = path_to_string(&value.path)?;
        Ok(DbDataDir { id: value.id, path })
    }
}

impl TryFrom<DataDir> for DbDataDir {
    type Error = eyre::Report;

    fn try_from(value: DataDir) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
