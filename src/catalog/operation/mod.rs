use std::path::PathBuf;

use eyre::Result;
use tracing::instrument;

use crate::model::repository::{self, pool::DbPool};

pub mod create_thumbnail;
pub mod package_video;
pub mod transcode_video;
