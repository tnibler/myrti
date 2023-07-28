use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

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
    Indexing(AJobHandle<IndexingJob>),
    Thumbnail(AJobHandle<ThumbnailJob>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    NotStarted,
    Running,
    Complete,
    Failed,
    Canceled,
}

pub struct JobHandle {
    pub cancel: CancellationToken,
    pub typed_handle: JobHandleType,
}

pub struct TypedJobHandle<T: Job> {
    pub status_rx: mpsc::Receiver<JobStatus>,
    pub join_handle: JoinHandle<T::Result>,
}

pub struct AJobHandle<T: Job> {
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
