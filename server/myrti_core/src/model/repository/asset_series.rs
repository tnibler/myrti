use eyre::{eyre, Context, Result};

use crate::model::{AssetId, AssetSeries, AssetSeriesId};

use super::{db::DbConn, schema};

#[tracing::instrument(skip(conn))]
pub fn create_series(conn: &mut DbConn, asset_ids: &[AssetId]) -> Result<AssetSeriesId> {
    use diesel::prelude::*;
    use schema::{Asset, AssetSeries};

    if asset_ids.is_empty() {
        return Err(eyre!("asset_ids can not be empty"));
    }

    conn.immediate_transaction(|conn| {
        let series_id = diesel::insert_into(AssetSeries::table)
            .values(AssetSeries::is_auto.eq(0))
            .returning(AssetSeries::series_id)
            .get_result(conn)
            .wrap_err("error inserting into table AssetSeries")?;

        let affected_rows = diesel::update(
            Asset::table.filter(
                Asset::asset_id
                    .eq_any(asset_ids.iter().map(|id| id.0))
                    // can not already be part of a series
                    .and(Asset::series_id.is_null()),
            ),
        )
        .set((
            Asset::series_id.eq(series_id),
            Asset::is_series_selection.eq(0),
        ))
        .execute(conn)
        .wrap_err("error updating table Asset")?;

        // WHERE Asset.series_id IS NULL prevented some rows from being changed
        if affected_rows != asset_ids.len() {
            return Err(eyre!("one or more assets were already part of a series"));
        }

        // TODO: remove this. series with no selection is a valid state, we just don't
        // handle it yet on the client
        let affected_rows = diesel::update(Asset::table.filter(Asset::asset_id.eq(asset_ids[0].0)))
            .set(Asset::is_series_selection.eq(1))
            .execute(conn)
            .wrap_err("error setting first asset to selection true")?;
        assert!(affected_rows == 1);
        Ok(AssetSeriesId(series_id))
    })
}

#[tracing::instrument(skip(conn))]
pub fn get_series_for_asset(conn: &mut DbConn, asset_id: AssetId) -> Result<Option<AssetSeries>> {
    use diesel::prelude::*;
    use schema::Asset;
    conn.transaction(|conn| {
        let (asset1, asset2) = diesel::alias!(Asset as asset1, Asset as asset2);
        // diesel::joinable!(asset1 -> asset2 (series_id));
        let rows: Vec<(i64, Option<i32>, Option<i64>)> = asset1
            .filter(
                asset1
                    .field(Asset::asset_id)
                    .eq(asset_id.0)
                    .and(asset1.field(Asset::series_id).is_not_null()),
            )
            .inner_join(
                asset2.on(asset1
                    .field(Asset::series_id)
                    .eq(asset2.field(Asset::series_id))),
            )
            .order_by(asset2.field(Asset::taken_date).desc())
            .select(asset2.fields((
                Asset::asset_id,
                Asset::is_series_selection,
                Asset::series_id,
            )))
            .load(conn)?;
        let series_id = if let Some(row) = rows.first() {
            AssetSeriesId(row.2.expect("was filtered not null"))
        } else {
            return Err(eyre!("Asset is not part of a series"));
        };
        let selection_indices: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter_map(|(idx, (_asset_id, is_selection, _series_id))| {
                let is_selection = is_selection.expect("was filtered not null") != 0;
                is_selection.then_some(idx)
            })
            .collect();
        let asset_ids = rows
            .into_iter()
            .map(|(asset_id, _, _)| AssetId(asset_id))
            .collect();
        Ok(Some(crate::model::AssetSeries {
            series_id,
            asset_ids,
            selection_indices,
        }))
    })
}
