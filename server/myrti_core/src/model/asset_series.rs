use crate::model::{AssetId, AssetSeriesId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetSeries {
    pub series_id: AssetSeriesId,
    pub asset_ids: Vec<AssetId>,
    pub selection_indices: Vec<usize>,
}
