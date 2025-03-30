use eyre::{eyre, Context, Result};

use crate::model::{AssetId, AssetSeriesId};

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
