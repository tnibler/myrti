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

use self::operation::create_thumbnail::CreateThumbnail;
use self::operation::package_video::PackageVideo;

/// An operation that alters the catalog state
#[derive(Debug, Clone)]
pub enum Operation {
    CreateThumbnail(Vec<CreateThumbnail>),
    PackageVideo(Vec<PackageVideo>),
}
