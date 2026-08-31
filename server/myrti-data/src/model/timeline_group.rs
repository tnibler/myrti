use chrono::{DateTime, Utc};

use super::TimelineGroupId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineGroup {
    pub id: TimelineGroupId,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub changed_at: DateTime<Utc>,
}
