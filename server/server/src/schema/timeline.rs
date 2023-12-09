use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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
    #[serde(rename = "type")]
    pub ty: TimelineGroupType,
    pub assets: Vec<AssetWithSpe>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
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

// Paging API
//
// problem: timeline is not strictly chronological, since groups are displayed together even if
// they're not necessarily contiguous.
// that means "continue from this last timestamp" is not a valid strategy for paging the timeline.
// solution: there are assets in the timeline where everything above has a strictly newer
// timestamp, which can be used as "synchronization points".
// The client could pass the last synchronization point date, and the last assets it has actually
// displayed. The server can then use the synch point to compute the rest of the timeline,
// and trim off the beginning until it reaches assets the clients doesn't have yet.
//
// An asset A is a synch point if the last asset of every group displayed above it has a date newer than A.
//
// so to start timeline at instant T with last displayed id I,
// check if I is in a group (only one)
// if yes get groupa, serve assets from that
// when done, first asset older than group display date
// => last date T not even needed?
