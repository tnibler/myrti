use chrono::{DateTime, Utc};
use diesel::prelude::*;
use eyre::{Context, Result, eyre};
use tracing::instrument;

use super::db_entity::{DbAsset, DbTimelineGroup};
use super::schema;
use super::util::{datetime_from_db_repr, datetime_to_db_repr};
use crate::db::DbConn;
use crate::model::{AssetBase, AssetId, TimelineGroup, TimelineGroupId};

pub fn get_timeline_group(conn: &mut DbConn, id: TimelineGroupId) -> Result<TimelineGroup> {
    use schema::TimelineGroup;

    let db_timeline_group: DbTimelineGroup = TimelineGroup::table.find(id.0).first(conn)?;
    db_timeline_group.try_into()
}

pub fn get_timeline_group_for_asset(
    conn: &mut DbConn,
    asset_id: AssetId,
) -> Result<Option<TimelineGroup>> {
    use schema::{AssetsInTimelineGroup, TimelineGroup};

    let db_timeline_group: Option<DbTimelineGroup> = AssetsInTimelineGroup::table
        .filter(AssetsInTimelineGroup::asset_id.eq(asset_id.0))
        .inner_join(TimelineGroup::table)
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

pub fn create_timeline_group(
    conn: &mut DbConn,
    ctg: CreateTimelineGroup,
) -> Result<TimelineGroupId> {
    use schema::{Asset, TimelineGroup, TimelineGroupItem};
    let now = Utc::now();
    let group_id = conn.immediate_transaction(|conn| {
        let group_id: i64 = diesel::insert_into(TimelineGroup::table)
            .values((
                TimelineGroup::name.eq(ctg.name),
                TimelineGroup::created_at.eq(datetime_to_db_repr(&now)),
                TimelineGroup::changed_at.eq(datetime_to_db_repr(&now)),
            ))
            .returning(TimelineGroup::timeline_group_id)
            .get_result(conn)
            .wrap_err("error inserting into TimelineGroup")?;

        for asset_id in &ctg.asset_ids {
            diesel::insert_into(TimelineGroupItem::table)
                .values((
                    TimelineGroupItem::group_id.eq(group_id),
                    TimelineGroupItem::asset_id.eq(asset_id.0),
                    TimelineGroupItem::series_id.eq(Asset::table
                        .find(asset_id.0)
                        .select(Asset::series_id)
                        .single_value()),
                ))
                .execute(conn)
                .wrap_err("error inserting into TimelineGroupItem")?;
        }
        Ok::<_, eyre::Error>(TimelineGroupId(group_id))
    })?;
    if let Err(err) = super::timeline::update_timeline_dirty(conn) {
        tracing::error!("Error updating dirty timeline:\n{:?}", err);
    }
    Ok(group_id)
}

pub fn get_newest_asset_date(
    conn: &mut DbConn,
    asset_ids: &[AssetId],
) -> Result<Option<DateTime<Utc>>> {
    use diesel::dsl::max;
    use schema::Asset;
    let asset_ids: Vec<i64> = asset_ids.iter().map(|id| id.0).collect();
    let max_date: Option<i64> = Asset::table
        .filter(Asset::asset_id.eq_any(asset_ids))
        .select(max(Asset::taken_date))
        .get_result(conn)?;
    max_date.map(datetime_from_db_repr).transpose()
}

#[instrument(skip(conn))]
pub fn add_assets_to_group(
    conn: &mut DbConn,
    group_id: TimelineGroupId,
    asset_ids: &[AssetId],
) -> Result<()> {
    use schema::{Asset, TimelineGroupItem};
    if asset_ids.is_empty() {
        return Ok(());
    }
    conn.immediate_transaction(|conn| {
        for asset_id in asset_ids {
            diesel::insert_into(TimelineGroupItem::table)
                .values((
                    TimelineGroupItem::group_id.eq(group_id.0),
                    TimelineGroupItem::asset_id.eq(asset_id.0),
                    TimelineGroupItem::series_id.eq(Asset::table
                        .find(asset_id.0)
                        .select(Asset::series_id)
                        .single_value()),
                ))
                .execute(conn)
                .wrap_err("error inserting into TimelineGroupItem")?;
        }
        Ok::<_, eyre::Error>(())
    })?;
    if let Err(err) = super::timeline::update_timeline_dirty(conn) {
        tracing::error!("Error updating dirty timeline:\n{:?}", err);
    }
    Ok(())
}

#[instrument(skip(conn))]
pub fn remove_assets_from_group(
    conn: &mut DbConn,
    group_id: TimelineGroupId,
    asset_ids: &[AssetId],
) -> Result<()> {
    use diesel::sql_types::BigInt;
    use schema::TimelineGroupItem;
    define_sql_function! { fn coalesce(x: BigInt, y: BigInt) -> BigInt; }

    if asset_ids.is_empty() {
        return Ok(());
    }
    conn.immediate_transaction(|conn| {
        let tgi_series = diesel::alias!(TimelineGroupItem as tgi_series);
        let tgi_ids: Vec<i64> = TimelineGroupItem::table
            .filter(
                TimelineGroupItem::group_id
                    .eq(group_id.0)
                    .and(TimelineGroupItem::asset_id.eq_any(asset_ids.iter().map(|id| id.0))),
            )
            .left_join(
                tgi_series.on(tgi_series
                    .field(TimelineGroupItem::series_id)
                    .eq(TimelineGroupItem::series_id)),
            )
            .select(
                coalesce(
                    TimelineGroupItem::timeline_group_item_id,
                    tgi_series.field(TimelineGroupItem::timeline_group_item_id),
                )
                .assume_not_null(),
            )
            .load(conn)
            .wrap_err("error querying table TimelineGroupItem")?;
        let affected_rows = diesel::delete(
            TimelineGroupItem::table
                .filter(TimelineGroupItem::timeline_group_item_id.eq_any(&tgi_ids)),
        )
        .execute(conn)?;
        if affected_rows < asset_ids.len() {
            Err(eyre!(
                "mismatch: not all asset_ids belonged to specified group_id"
            ))
        } else {
            Ok(())
        }
    })?;
    if let Err(err) = super::timeline::update_timeline_dirty(conn) {
        tracing::error!("Error updating dirty timeline:\n{:?}", err);
    }
    Ok(())
}

#[instrument(skip(conn))]
pub fn get_assets_in_group(conn: &mut DbConn, group_id: TimelineGroupId) -> Result<Vec<AssetBase>> {
    use schema::{Asset, AssetsInTimelineGroup};
    let db_assets: Vec<DbAsset> = AssetsInTimelineGroup::table
        .filter(AssetsInTimelineGroup::group_id.eq(group_id.0))
        .inner_join(Asset::table)
        .select(DbAsset::as_select())
        .load(conn)?;
    db_assets
        .into_iter()
        .map(AssetBase::try_from)
        .collect::<Result<Vec<_>>>()
}
