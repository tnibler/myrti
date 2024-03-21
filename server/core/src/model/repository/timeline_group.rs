use chrono::{DateTime, Utc};
use diesel::prelude::*;
use eyre::Result;
use tracing::instrument;

use crate::model::{
    repository::db_entity::{DbAsset, DbTimelineGroup},
    timeline_group::TimelineGroup,
    util::datetime_to_db_repr,
    Asset, AssetId, TimelineGroupId,
};

use super::{db::DbConn, schema};

#[instrument(skip(conn), level = "trace")]
pub fn get_timeline_group(conn: &mut DbConn, id: TimelineGroupId) -> Result<TimelineGroup> {
    use schema::TimelineGroup;

    let db_timeline_group: DbTimelineGroup = TimelineGroup::table.find(id.0).first(conn)?;
    db_timeline_group.try_into()
}

#[instrument(skip(conn), level = "trace")]
pub fn get_timeline_group_album_for_asset(
    conn: &mut DbConn,
    asset_id: AssetId,
) -> Result<Option<TimelineGroup>> {
    use schema::{Asset, TimelineGroup, TimelineGroupEntry};

    let db_timeline_group: Option<DbTimelineGroup> = TimelineGroupEntry::table
        .filter(TimelineGroupEntry::asset_id.eq(asset_id.0))
        .inner_join(TimelineGroup::table)
        .inner_join(Asset::table)
        .select(DbTimelineGroup::as_select())
        .first(conn)
        .optional()?;
    db_timeline_group
        .map(|db_tlg| db_tlg.try_into())
        .transpose()
}

#[derive(Debug, Clone)]
pub struct CreateTimelineGroup {
    pub name: Option<String>,
    pub display_date: DateTime<Utc>,
    pub asset_ids: Vec<AssetId>,
}

#[instrument(skip(conn), level = "trace")]
pub fn create_timeline_group(
    conn: &mut DbConn,
    ctg: CreateTimelineGroup,
) -> Result<TimelineGroupId> {
    use schema::{TimelineGroup, TimelineGroupEntry};
    let now = Utc::now();
    conn.transaction(|conn| {
        let group_id: i64 = diesel::insert_into(TimelineGroup::table)
            .values((
                TimelineGroup::name.eq(ctg.name),
                TimelineGroup::display_date.eq(datetime_to_db_repr(&ctg.display_date)),
                TimelineGroup::created_at.eq(datetime_to_db_repr(&now)),
                TimelineGroup::changed_at.eq(datetime_to_db_repr(&now)),
            ))
            .returning(TimelineGroup::timeline_group_id)
            .get_result(conn)?;

        for asset_id in &ctg.asset_ids {
            diesel::insert_into(TimelineGroupEntry::table)
                .values((
                    TimelineGroupEntry::group_id.eq(group_id),
                    TimelineGroupEntry::asset_id.eq(asset_id.0),
                ))
                .execute(conn)?;
        }
        Ok(TimelineGroupId(group_id))
    })
}

#[instrument(skip(conn), level = "trace")]
pub fn add_assets_to_group(
    conn: &mut DbConn,
    group_id: TimelineGroupId,
    asset_ids: &[AssetId],
) -> Result<()> {
    use schema::TimelineGroupEntry;
    if asset_ids.is_empty() {
        return Ok(());
    }
    conn.transaction(|conn| {
        for asset_id in asset_ids {
            diesel::insert_into(TimelineGroupEntry::table)
                .values((
                    TimelineGroupEntry::group_id.eq(group_id.0),
                    TimelineGroupEntry::asset_id.eq(asset_id.0),
                ))
                .execute(conn)?;
        }
        Ok(())
    })
}

#[instrument(skip(conn), level = "trace")]
pub fn get_assets_in_group(conn: &mut DbConn, group_id: TimelineGroupId) -> Result<Vec<Asset>> {
    use schema::{Asset, TimelineGroupEntry};
    let db_assets: Vec<DbAsset> = TimelineGroupEntry::table
        .filter(TimelineGroupEntry::group_id.eq(group_id.0))
        .inner_join(Asset::table)
        .select(DbAsset::as_select())
        .load(conn)?;
    db_assets
        .into_iter()
        .map(|db_asset| db_asset.try_into())
        .collect::<Result<Vec<_>>>()
}
