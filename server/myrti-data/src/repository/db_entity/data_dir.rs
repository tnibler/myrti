use crate::model::{DataDir, DataDirId};
use camino::Utf8PathBuf as PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbDataDir {
    pub id: DataDirId,
    pub path: String,
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
        Ok(DbDataDir {
            id: value.id,
            path: value.path.to_owned().into(),
        })
    }
}

impl TryFrom<DataDir> for DbDataDir {
    type Error = eyre::Report;

    fn try_from(value: DataDir) -> Result<Self, Self::Error> {
        (&value).try_into()
    }
}
