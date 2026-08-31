use camino::Utf8PathBuf as PathBuf;

use super::DataDirId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    pub id: DataDirId,
    pub path: PathBuf,
}
