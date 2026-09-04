use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use myrti_data::model;

use crate::schema::AssetId;

use super::AssetSeriesId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetSeries {
    pub id: AssetSeriesId,
    pub asset_ids: Vec<AssetId>,
    pub selection_indices: Vec<usize>,
}

impl AssetSeries {
    pub fn from_model(value: &model::AssetSeries) -> AssetSeries {
        AssetSeries {
            id: value.series_id.into(),
            asset_ids: value.asset_ids.iter().map(AssetId::from).collect(),
            selection_indices: value.selection_indices.clone(),
        }
    }
}
