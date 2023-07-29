use crate::{
    model::AssetBase,
    repository::{self, pool::DbPool},
};
use eyre::Result;

pub async fn assets_without_thumbnails(pool: &DbPool) -> Result<Vec<AssetBase>> {
    repository::asset::get_assets_with_missing_thumbnail(pool, None).await
}
