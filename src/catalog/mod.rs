//! The asset catalog is the primary state of the application, i.e. what assets we have,
//! and what associated resources like thumbnails, alternative representations
//! in different codecs or reverse geocoding results.
//! The database is the single source of truth for the current application state,
//! but most things in it primarily point to files.
//!
//! To determine what the application should do next (generate thumbnails, transcode a video),
//! we apply the set of rules against the current catalog,
//! which gives us an Operation that can be applied to the state,
//! altering the database and producing other side effects,
//! namely creating files in the filesystem, running ffmpeg and so on.
//! Altering the database state and running an Operation's side effect
//! are separate to create a state machine that's reasonably testable without any IO or intensive
//! compute.

pub mod encoding_target;
pub mod operation;
pub mod rules;

use std::path::PathBuf;

use crate::model::{DataDirId, ResourceFileId};

use self::operation::create_thumbnail::CreateThumbnail;
use self::operation::package_video::PackageVideo;

/// An operation that alters the catalog state
///
/// Generic over the type of resource path it refers to,
/// because the rules determining operations to perform are not concerned with
/// where the resulting file resources are actually stored.
/// Resource files/directories may be located in any data directory,
/// so rules emit Operations with paths relative to an unspecified resource directory.
///
/// When applying and especially running the side effects of an Operation,
/// these relative paths are resolved to be relative to a specific resource directory (with an
/// actual path on disk).
/// For example, for the PackageVideo operation, the transcoding output path
/// will be resolved relative to the video asset's dash_resource_dir column.
/// This resolved path may be in an already existing resource directory
/// or a newly created one, which have to be handled separately
/// (when creating a new resource file/directory, inserting it and then altering
/// an asset record to point to it must be done in the same transaction).
#[derive(Debug, Clone)]
pub enum Operation<P: ResourcePath> {
    CreateThumbnail(Vec<CreateThumbnail<P>>),
    PackageVideo(Vec<PackageVideo<P>>),
}

#[derive(Debug, Clone)]
pub enum ResolvedResourcePath {
    Existing(ResolvedExistingResourcePath),
    New(ResolvedNewResourcePath),
}

#[derive(Debug, Clone)]
pub struct PathInResourceDir(pub PathBuf);

#[derive(Debug, Clone)]
pub struct ResolvedExistingResourcePath {
    pub resource_dir_id: ResourceFileId,
    pub path_in_resource_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedNewResourcePath {
    pub data_dir_id: DataDirId,
    pub path_in_data_dir: PathBuf,
}

pub trait ResourcePath {}

impl ResourcePath for ResolvedResourcePath {}
impl ResourcePath for PathInResourceDir {}

impl From<PathBuf> for PathInResourceDir {
    fn from(value: PathBuf) -> Self {
        Self(value)
    }
}
