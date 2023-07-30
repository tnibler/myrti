use crate::job::{
    indexing_job::{IndexingJob, IndexingJobResult},
    thumbnail_job::{ThumbnailJob, ThumbnailJobResult},
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
    pub status_rx: mpsc::Receiver<JobStatus>,
    pub join_handle: JoinHandle<JobResultType>,
}

#[async_trait]
pub trait Job {
    type Result: Debug;

    fn start(self) -> JobHandle
    where
        Self: Sized;
}
