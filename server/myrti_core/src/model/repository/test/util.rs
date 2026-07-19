use proptest::prelude::*;

use crate::model::{
    Asset, AssetBase, AssetId, AssetRootDirId, AssetSpe, AssetType, CreateAsset, CreateAssetBase,
    CreateAssetSpe, Image, TimelineGroup, TimelineGroupId, Video, VideoAsset,
    repository::{self, db::DbConn, timeline_group::CreateTimelineGroup},
};

/// Inserts asset and returns them in the same order, with asset_id set
/// For VideoAssets, it uses an empty string as ffprobe_output
pub fn prop_insert_create_test_assets(
    conn: &mut DbConn,
    assets: &[CreateAsset],
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
    asset: &CreateAsset,
) -> Result<Asset, TestCaseError> {
    let asset_insert_result = repository::asset::create_asset(conn, asset.clone());
    prop_assert!(
        asset_insert_result.is_ok(),
        "Inserting Asset returned error: {}",
        asset_insert_result.unwrap_err()
    );
    let asset_id = asset_insert_result.unwrap();
    let retrieved = repository::asset::get_asset(conn, asset_id);
    prop_assert!(
        retrieved.is_ok(),
        "retrieve failed: {:?}",
        retrieved.unwrap_err()
    );
    let retrieved = retrieved.unwrap();
    Ok(make_created_asset(asset, &retrieved))
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

pub fn set_assets_root_dir(
    assets: Vec<CreateAsset>,
    root_dir_id: AssetRootDirId,
) -> Vec<CreateAsset> {
    assets
        .into_iter()
        .map(|asset| CreateAsset {
            base: CreateAssetBase {
                root_dir_id,
                ..asset.base
            },
            spe: asset.spe,
        })
        .collect()
}

pub fn set_asset_root_dir(asset: CreateAsset, root_dir_id: AssetRootDirId) -> CreateAsset {
    CreateAsset {
        base: CreateAssetBase {
            root_dir_id,
            ..asset.base
        },
        ..asset
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

pub fn make_created_asset(create: &CreateAsset, inserted: &Asset) -> Asset {
    Asset {
        base: AssetBase {
            ty: match &create.spe {
                CreateAssetSpe::Image(_) => AssetType::Image,
                CreateAssetSpe::Video(_) => AssetType::Video,
            },
            root_dir_id: create.base.root_dir_id,
            file_type: create.base.file_type.clone(),
            file_path: create.base.file_path.clone(),
            is_hidden: create.base.is_hidden,
            taken_date: create.base.taken_date,
            timestamp_info: create.base.timestamp_info.clone(),
            size: create.base.size,
            rotation_correction: create.base.rotation_correction,
            gps_coordinates: create.base.gps_coordinates,

            id: inserted.base.id,
            added_at: inserted.base.added_at,
            hash: inserted.base.hash,
        },
        sp: match (&create.spe, &inserted.sp) {
            (CreateAssetSpe::Image(create_img), AssetSpe::Image(img)) => AssetSpe::Image(Image {
                image_asset_id: img.image_asset_id,
                image_format_name: create_img.image_format_name.clone(),
            }),
            (CreateAssetSpe::Video(create_vid), AssetSpe::Video(vid)) => AssetSpe::Video(Video {
                video_asset_id: vid.video_asset_id,
                video_codec_name: create_vid.video_codec_name.clone(),
                video_bitrate: create_vid.video_bitrate,
                audio_codec_name: create_vid.audio_codec_name.clone(),
                is_original_streamable: create_vid.is_original_streamable,
                max_iframe_interval: create_vid.max_iframe_interval,
                frame_rate: create_vid.frame_rate,
            }),
            _ => panic!(),
        },
    }
}
