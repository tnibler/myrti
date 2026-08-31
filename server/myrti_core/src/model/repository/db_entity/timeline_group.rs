use diesel::{Queryable, Selectable};

use crate::model::{TimelineGroup, TimelineGroupId, util::datetime_from_db_repr};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::TimelineGroup)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbTimelineGroup {
    pub timeline_group_id: i64,
    pub name: Option<String>,
    pub created_at: i64,
    pub changed_at: i64,
}

impl TryFrom<DbTimelineGroup> for TimelineGroup {
    type Error = eyre::Report;

    fn try_from(value: DbTimelineGroup) -> Result<TimelineGroup, Self::Error> {
        let created_at = datetime_from_db_repr(value.created_at)?;
        let changed_at = datetime_from_db_repr(value.changed_at)?;
        Ok(TimelineGroup {
            id: TimelineGroupId(value.timeline_group_id),
            name: value.name,
            created_at,
            changed_at,
        })
    }
}
