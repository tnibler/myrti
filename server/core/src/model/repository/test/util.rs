use proptest::prelude::*;

use crate::model::{
    repository::{self, db::DbConn, timeline_group::CreateTimelineGroup},
    Asset, AssetBase, AssetId, AssetRootDirId, AssetSpe, TimelineGroup, TimelineGroupId,
    VideoAsset,
};

/// Inserts asset and returns them in the same order, with asset_id set
/// For VideoAssets, it uses an empty string as ffprobe_output
pub fn prop_insert_create_test_assets(
    conn: &mut DbConn,
    assets: &[Asset],
) -> Result<Vec<Asset>, TestCaseError> {
    let mut assets_with_ids: Vec<Asset> = Vec::default();
    for asset in assets {
        let asset_with_id = prop_insert_create_test_asset(conn, asset)?;
        assets_with_ids.push(asset_with_id);
    }
    Ok(assets_with_ids)
}

pub fn prop_insert_create_test_asset(
    conn: &mut DbConn,
    asset: &Asset,
) -> Result<Asset, TestCaseError> {
    let ffprobe_output: Option<&[u8]> = match &asset.sp {
        AssetSpe::Video(_video) => Some(&[]),
        _ => None,
    };
    #[allow(deprecated)]
    let asset_insert_result = repository::asset::insert_asset(conn, asset, ffprobe_output);
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
    Ok(asset_with_id)
}

/// Inserts empty albums, then adds assets to them
/// returns albums in same order with album_id set
pub fn prop_insert_timeline_groups_add_assets(
    conn: &mut DbConn,
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
            prop_insert_timeline_group_add_assets(conn, group, assets.iter().copied())?;
        groups_with_ids.push(group_with_id);
    }
    Ok(groups_with_ids)
}

pub fn prop_insert_timeline_group_add_assets(
    conn: &mut DbConn,
    group: &TimelineGroup,
    asset_ids: impl Iterator<Item = AssetId>,
) -> Result<TimelineGroup, TestCaseError> {
    let create_group = CreateTimelineGroup {
        name: group.name.clone(),
        display_date: group.display_date,
        asset_ids: Vec::new(),
    };
    // initial creation with assets to insert right away not tested here
    let group_insert_result = repository::timeline_group::create_timeline_group(conn, create_group);
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
    prop_assert_ne!(group_with_id.id, TimelineGroupId(0));
    let append_result = repository::timeline_group::add_assets_to_group(
        conn,
        group_with_id.id,
        &asset_ids.collect::<Vec<_>>(),
    );
    prop_assert!(
        append_result.is_ok(),
        "Appending to Album returned error: {:?}",
        append_result.unwrap_err()
    );
    Ok(group_with_id)
}

pub fn set_assets_root_dir(assets: Vec<Asset>, root_dir_id: AssetRootDirId) -> Vec<Asset> {
    assets
        .into_iter()
        .map(|asset| Asset {
            base: AssetBase {
                root_dir_id,
                ..asset.base
            },
            sp: asset.sp,
        })
        .collect()
}

pub fn set_asset_root_dir(asset: Asset, root_dir_id: AssetRootDirId) -> Asset {
    Asset {
        base: AssetBase {
            root_dir_id,
            ..asset.base
        },
        sp: asset.sp,
    }
}

pub fn set_video_asset_root_dir(asset: VideoAsset, root_dir_id: AssetRootDirId) -> VideoAsset {
    VideoAsset {
        base: AssetBase {
            root_dir_id,
            ..asset.base
        },
        video: asset.video,
    }
}
