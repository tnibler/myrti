use crate::model::DataDirId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbDataDir {
    pub id: DataDirId,
    pub path: String,
}
