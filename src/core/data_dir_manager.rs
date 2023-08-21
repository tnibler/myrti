use std::path::{Path, PathBuf};

use crate::{
    model::{repository, DataDirId},
    repository::pool::DbPool,
};
use eyre::Result;

/// Decides where to put new resource files (thumbnails, transcoded media etc..).
///
/// In the future, it will also shuffle existing files around based on access
/// patterns and storage speeds for different data dirs (e.g. fast ssd, slow hdd or even cloud)
pub struct DataDirManager {
    pool: DbPool,
}

impl DataDirManager {
    pub fn new(pool: DbPool) -> DataDirManager {
        DataDirManager { pool }
    }

    pub async fn new_thumbnail_file(&self, file_name: &Path) -> Result<PathBuf> {
        let thumbnail_path = PathBuf::from("thumbnails");
        let data_dir = repository::data_dir::get_random_data_dir(&self.pool).await?;
        let complete_dir_path = data_dir.path.join(&thumbnail_path);
        // FIXME this might belong somewhere else
        tokio::fs::create_dir_all(&complete_dir_path).await.unwrap();
        Ok(complete_dir_path.join(file_name))
    }

    pub async fn new_dash_dir(&self, dir_name: &str) -> Result<PathBuf> {
        let dash_path = PathBuf::from("dash");
        let data_dir = repository::data_dir::get_random_data_dir(&self.pool).await?;
        let complete_path = data_dir.path.join(&dash_path).join(dir_name);
        tokio::fs::create_dir_all(&complete_path).await.unwrap();
        Ok(complete_path)
    }
}

/// Result of calls to DataDirManager, represents a not yet created
/// file in a DataDir. Callers of DataDirManager have to insert this
/// into the db once they've successfully written to the path on disk
/// id represents
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewResourceFile {
    pub data_dir_id: DataDirId,
    pub data_dir_path: PathBuf,
    pub path_in_data_dir: PathBuf,
}

impl NewResourceFile {
    pub fn path_on_disk(&self) -> PathBuf {
        self.data_dir_path.join(&self.path_in_data_dir)
    }
}
