use crate::{
    job::{
        indexing_job::{IndexingJob, IndexingJobParams, IndexingJobResult},
        thumbnail_job::{ThumbnailJob, ThumbnailJobParams},
    },
    model::AssetId,
};
use async_trait::async_trait;
use eyre::Result;
use std::fmt::{Debug, Display};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

impl Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("JobId({})", self.0))
    }
}

#[derive(Debug)]
pub enum JobResultType {
    Indexing(<IndexingJob as Job>::Result),
    Thumbnail(<ThumbnailJob as Job>::Result),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobType {
    Indexing { params: IndexingJobParams },
    Thumbnail { params: ThumbnailJobParams },
}

impl Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobType::Indexing { params } => "Indexing",
            JobType::Thumbnail { params } => "Thumbnail",
        };
        write!(f, "{}", s)
    }
}

impl Display for JobResultType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobResultType::Indexing(_) => "Indexing",
            JobResultType::Thumbnail(_) => "Thumbnail",
        };
        write!(f, "{}Result", s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobProgress {
    pub percent: Option<i32>,
    pub description: String,
}

// this doesnt actually need to exist in this way.
// job sends progress updates,
// monitor waits for result and can decide itself if the job failed or not
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    NotStarted,
    Running(JobProgress),
    Complete,
    Failed { msg: String },
    Cancelled,
}

pub struct JobHandle {
    pub cancel: CancellationToken,
    pub progress_rx: mpsc::Receiver<JobProgress>,
    pub join_handle: JoinHandle<JobResultType>,
}

#[async_trait]
pub trait Job {
    type Result: Debug;

    fn start(self) -> JobHandle
    where
        Self: Sized;
}
