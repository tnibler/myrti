use eyre::bail;
use serde::Serialize;

use crate::core;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum JobType {
    Indexing,
    Thumbnail,
    VideoPackaging,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum JobStatus {
    NotStarted,
    Running,
    Complete,
    CompleteWithErrors,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JobInfo {
    pub id: JobId,
    #[serde(rename = "type")]
    pub ty: JobType,
    pub status: Option<JobStatus>,
}

impl From<core::job::JobId> for JobId {
    fn from(value: core::job::JobId) -> Self {
        JobId(value.0.to_string())
    }
}

impl TryFrom<JobId> for core::job::JobId {
    type Error = eyre::Report;
    fn try_from(value: JobId) -> Result<Self, Self::Error> {
        match value.0.parse::<u64>() {
            Ok(id) => Ok(core::job::JobId(id)),
            Err(_) => bail!("Invalid JobId {}", value.0),
        }
    }
}

pub fn api_job_type(job_type: &core::job::JobType) -> JobType {
    match job_type {
        core::job::JobType::Indexing { params: _ } => JobType::Indexing,
        core::job::JobType::Thumbnail { params: _ } => JobType::Thumbnail,
        core::job::JobType::VideoPackaging { params: _ } => JobType::VideoPackaging,
    }
}

pub fn api_job_status(job_status: &core::job::JobStatus) -> JobStatus {
    match job_status {
        core::job::JobStatus::NotStarted => JobStatus::NotStarted,
        core::job::JobStatus::Running(_) => JobStatus::Running,
        core::job::JobStatus::Complete => JobStatus::Complete,
        core::job::JobStatus::CompleteWithErrors => JobStatus::CompleteWithErrors,
        core::job::JobStatus::Failed { msg: _ } => JobStatus::Failed,
        core::job::JobStatus::Cancelled => JobStatus::Cancelled,
    }
}
