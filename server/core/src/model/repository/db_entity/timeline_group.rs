use crate::model::{util::datetime_from_db_repr, TimelineGroupId, TimelineGroup};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DbTimelineGroup {
    pub id: TimelineGroupId,
    pub name: Option<String>,
    pub display_date: i64,
    pub created_at: i64,
    pub changed_at: i64,
}

impl TryFrom<&DbTimelineGroup> for TimelineGroup {
    type Error = eyre::Report;

    fn try_from(value: &DbTimelineGroup) -> Result<TimelineGroup, Self::Error> {
        let display_date = datetime_from_db_repr(value.display_date)?;
        let created_at = datetime_from_db_repr(value.created_at)?;
        let changed_at = datetime_from_db_repr(value.changed_at)?;
        Ok(TimelineGroup {
            id: value.id,
            name: value.name.clone(),
            display_date,
            created_at,
            changed_at,
        })
    }
}

impl TryFrom<DbTimelineGroup> for TimelineGroup {
    type Error = eyre::Report;

    fn try_from(value: DbTimelineGroup) -> Result<TimelineGroup, Self::Error> {
        (&value).try_into()
    }
}
