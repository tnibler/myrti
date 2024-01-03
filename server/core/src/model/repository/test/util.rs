use proptest::prelude::*;

use crate::model::{
    repository::{self, pool::DbPool, timeline_group::CreateTimelineGroup},
    Asset, AssetBase, AssetId, AssetSpe, TimelineGroup, TimelineGroupId,
};

/// Inserts asset and returns them in the same order, with asset_id set
/// For VideoAssets, it uses an empty string as ffprobe_output
pub async fn prop_insert_create_test_assets(
    pool: &DbPool,
    assets: &[Asset],
) -> Result<Vec<Asset>, TestCaseError> {
    let mut assets_with_ids: Vec<Asset> = Vec::default();
    for asset in assets {
        let ffprobe_output: Option<&[u8]> = match &asset.sp {
            AssetSpe::Video(_video) => Some(&[]),
            _ => None,
        };
        #[allow(deprecated)]
        let asset_insert_result =
            repository::asset::insert_asset(&pool, &asset, ffprobe_output).await;
        prop_assert!(
            asset_insert_result.is_ok(),
            "Inserting Asset returned error: {}",
            asset_insert_result.unwrap_err()
        );
        let asset_id = asset_insert_result.unwrap();
        let asset_with_id = Asset {
            base: AssetBase {
                id: asset_id,
                ..asset.base.clone()
            },
            ..asset.clone()
        };
        prop_assert_ne!(asset_with_id.base.id, AssetId(0));
        assets_with_ids.push(asset_with_id.into());
    }
    Ok(assets_with_ids)
}

/// Inserts empty albums, then adds assets to them
/// returns albums in same order with album_id set
pub async fn prop_insert_timeline_groups_add_assets(
    pool: &DbPool,
    assets: &[Asset],
    groups_asset_idxs: &[(TimelineGroup, Vec<prop::sample::Index>)],
) -> Result<Vec<TimelineGroup>, TestCaseError> {
    let mut groups_with_ids: Vec<TimelineGroup> = Vec::default();
    for (group, asset_idxs) in groups_asset_idxs {
        let assets: Vec<AssetId> = asset_idxs
            .iter()
            .map(|idx| idx.get(assets).base.id)
            .collect();
        let group_with_id =
            prop_insert_timeline_group_add_assets(pool, group, assets.iter().copied()).await?;
        groups_with_ids.push(group_with_id);
    }
    Ok(groups_with_ids)
}

pub async fn prop_insert_timeline_group_add_assets(
    pool: &DbPool,
    group: &TimelineGroup,
    asset_ids: impl Iterator<Item = AssetId>,
) -> Result<TimelineGroup, TestCaseError> {
    let create_group = CreateTimelineGroup {
        name: group.name.clone(),
        display_date: group.display_date,
        asset_ids: Vec::new(),
    };
    // initial creation with assets to insert right away not tested here
    let group_insert_result =
        repository::timeline_group::create_timeline_group(pool, create_group).await;
    prop_assert!(
        group_insert_result.is_ok(),
        "Inserting TimelineGroup returned error: {}",
        group_insert_result.unwrap_err()
    );
    let group_id = group_insert_result.unwrap();
    let group_with_id = TimelineGroup {
        id: group_id,
        ..group.clone()
    };
    let mut tx = pool.begin().await.unwrap();
    prop_assert_ne!(group_with_id.id, TimelineGroupId(0));
    let append_result = repository::timeline_group::add_assets_to_group(
        tx.as_mut(),
        group_with_id.id,
        &asset_ids.collect::<Vec<_>>(),
    )
    .await;
    prop_assert!(
        append_result.is_ok(),
        "Appending to Album returned error: {:?}",
        append_result.unwrap_err()
    );
    let commit_result = tx.commit().await;
    prop_assert!(
        commit_result.is_ok(),
        "Committing transaction returned error: {:?}",
        commit_result.unwrap_err()
    );
    Ok(group_with_id)
}
