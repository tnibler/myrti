use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use super::asset::AssetWithSpe;

/// Response for a request for the next part of the timeline to display
///
/// `groups` are always whole, not sliced in the middle. Either TimelineGroup or Day
/// `date` is the date before queries are made
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineChunk {
    pub date: DateTime<Utc>,
    pub changed_since_last_fetch: bool,
    pub groups: Vec<TimelineGroup>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineGroup {
    #[serde(flatten)]
    pub ty: TimelineGroupType,
    pub assets: Vec<AssetWithSpe>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum TimelineGroupType {
    #[serde(rename_all = "camelCase")]
    Day { date: NaiveDate },
    #[serde(rename_all = "camelCase")]
    Group {
        group_title: String,
        group_start_date: DateTime<Utc>,
        group_end_date: DateTime<Utc>,
        group_id: String,
    },
}
