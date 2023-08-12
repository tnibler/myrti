use std::path::PathBuf;

use eyre::Result;
use tracing::instrument;

use crate::model::repository::{self, pool::DbPool};

use super::{ResolvedExistingResourcePath, ResolvedNewResourcePath, ResolvedResourcePath};

pub mod create_thumbnail;
pub mod package_video;
pub mod transcode_video;

#[instrument(skip(pool))]
async fn resource_path_on_disk(
    pool: &DbPool,
    resolved_resource_path: &ResolvedResourcePath,
) -> Result<PathBuf> {
    match resolved_resource_path {
        ResolvedResourcePath::Existing(ResolvedExistingResourcePath {
            resource_dir_id,
            path_in_resource_dir,
        }) => {
            let resource_dir_path =
                repository::resource_file::get_resource_file_resolved(pool, *resource_dir_id)
                    .await?;
            Ok(resource_dir_path.path_on_disk().join(path_in_resource_dir))
        }
        ResolvedResourcePath::New(ResolvedNewResourcePath {
            data_dir_id,
            path_in_data_dir,
        }) => {
            let data_dir_path = repository::data_dir::get_data_dir(pool, *data_dir_id).await?;
            Ok(data_dir_path.path.join(path_in_data_dir))
        }
    }
}
