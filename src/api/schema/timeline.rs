use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::Asset;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineChunk {
    pub date: DateTime<Utc>,
    pub changed_since_last_fetch: bool,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineRequest {
    pub start: Option<String>,
    pub max_count: i32,
    pub last_fetch: String,
}
