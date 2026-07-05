use crate::model::AssetId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetSeries {
    pub asset_ids: Vec<AssetId>,
    pub selection_indices: Vec<usize>,
}
