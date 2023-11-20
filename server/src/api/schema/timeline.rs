use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::Asset;

/// Response for a request for the next part of the timeline to display
///
/// `groups` are always whole, not sliced in the middle. Either TimelineGroup or Day
/// `date` is the date before queries are made
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
    // TODO should this really by Utc and not NaiveDatempty asset vecs are filtered out above
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
    pub last_asset_id: Option<String>,
    pub max_count: i32,
    pub last_fetch: String,
}
