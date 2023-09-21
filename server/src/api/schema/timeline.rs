use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::Asset;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineChunk {
    pub date: DateTime<Utc>,
    pub changed_since_last_fetch: bool,
    pub groups: Vec<TimelineGroup>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineGroup {
    #[serde(rename = "type")]
    pub ty: TimelineGroupType,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineGroupType {
    Day(DateTime<Utc>),
    Group {
        title: String,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineRequest {
    pub start: Option<String>,
    pub start_id: Option<String>,
    pub max_count: i32,
    pub last_fetch: String,
}
