use diesel::{Queryable, Selectable};

use crate::model::{util::datetime_from_db_repr, TimelineGroup, TimelineGroupId};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = super::super::schema::TimelineGroup)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbTimelineGroup {
    pub timeline_group_id: i64,
    pub name: Option<String>,
    pub display_date: i64,
    pub created_at: i64,
    pub changed_at: i64,
}

impl TryFrom<DbTimelineGroup> for TimelineGroup {
    type Error = eyre::Report;

    fn try_from(value: DbTimelineGroup) -> Result<TimelineGroup, Self::Error> {
        let display_date = datetime_from_db_repr(value.display_date)?;
        let created_at = datetime_from_db_repr(value.created_at)?;
        let changed_at = datetime_from_db_repr(value.changed_at)?;
        Ok(TimelineGroup {
            id: TimelineGroupId(value.timeline_group_id),
            name: value.name,
            display_date,
            created_at,
            changed_at,
        })
    }
}
