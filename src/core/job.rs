use std::fmt::Debug;

use async_trait::async_trait;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::job::{indexing_job::IndexingJob, thumbnail_job::ThumbnailJob};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

pub enum JobType {
    Indexing(IndexingJob),
    Thumbnail(ThumbnailJob),
}

pub enum JobHandleType {
    Indexing(JobHandle<IndexingJob>),
    Thumbnail(JobHandle<ThumbnailJob>),
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
    Canceled,
}

pub struct JobHandle<T: Job> {
    pub cancel: CancellationToken,
    pub status_rx: mpsc::Receiver<JobStatus>,
    pub join_handle: JoinHandle<T::Result>,
}

#[async_trait]
pub trait Job {
    type Result: Debug;

    fn start(self) -> JobHandleType
    where
        Self: Sized;
}
