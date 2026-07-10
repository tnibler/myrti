use camino::Utf8PathBuf as PathBuf;
use chrono::{Days, Months};
use claims::{assert_err, assert_ok};

use crate::model::{
    repository::{self, timeline_group::CreateTimelineGroup},
    AssetId, AssetRootDir, AssetRootDirId, CreateAsset, CreateAssetBase, CreateAssetImage,
    CreateAssetSpe, Size, TimestampInfo,
};

use super::*;

#[test]
fn add_remove_assets_timeline_group() {
    let mut conn = super::db::open_in_memory_and_migrate();
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir
    ));
    let asset = CreateAsset {
        spe: CreateAssetSpe::Image(CreateAssetImage {
            image_format_name: "jpeg".into(),
        }),
        base: CreateAssetBase {
            root_dir_id,
            file_type: "jpeg".to_owned(),
            file_path: PathBuf::from("image.jpg"),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1024,
            },
            is_hidden: false,
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset2 = CreateAsset {
        spe: CreateAssetSpe::Image(CreateAssetImage {
            image_format_name: "jpeg".into(),
        }),
        base: CreateAssetBase {
            root_dir_id,
            file_type: "jpeg".to_owned(),
            file_path: PathBuf::from("image2.jpg"),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1024,
            },
            is_hidden: false,
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset3 = CreateAsset {
        base: CreateAssetBase {
            file_path: PathBuf::from("image3.jpg"),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(3))
                .unwrap(),
            ..asset2.base.clone()
        },
        ..asset2.clone()
    };
    let asset4 = CreateAsset {
        base: CreateAssetBase {
            file_path: PathBuf::from("image4.jpg"),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(3))
                .unwrap()
                .checked_sub_days(Days::new(4))
                .unwrap(),
            ..asset2.base.clone()
        },
        ..asset2.clone()
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&mut conn, asset.clone()));
    let asset2_id = assert_ok!(repository::asset::create_asset(&mut conn, asset2.clone()));
    let asset3_id = assert_ok!(repository::asset::create_asset(&mut conn, asset3.clone()));
    let asset4_id = assert_ok!(repository::asset::create_asset(&mut conn, asset4.clone()));

    let group = CreateTimelineGroup {
        name: Some("group1".into()),
        display_date: utc_now_millis_zero(),
        asset_ids: Vec::new(),
    };
    let group2 = CreateTimelineGroup {
        name: Some("group2".into()),
        display_date: utc_now_millis_zero()
            .checked_sub_months(Months::new(2))
            .unwrap(),
        asset_ids: Vec::new(),
    };
    let group_id = assert_ok!(repository::timeline_group::create_timeline_group(
        &mut conn, group
    ));
    let group2_id = assert_ok!(repository::timeline_group::create_timeline_group(
        &mut conn, group2
    ));
    assert_ok!(repository::timeline_group::add_assets_to_group(
        &mut conn,
        group_id,
        &[asset_id]
    ));
    let _ = assert_err!(repository::timeline_group::add_assets_to_group(
        &mut conn,
        group2_id,
        &[asset_id]
    ));

    let ret_group: Vec<AssetId> = assert_ok!(repository::timeline_group::get_assets_in_group(
        &mut conn, group_id
    ))
    .into_iter()
    .map(|asset| asset.base.id)
    .collect();
    let ret_group2: Vec<AssetId> = assert_ok!(repository::timeline_group::get_assets_in_group(
        &mut conn, group2_id
    ))
    .into_iter()
    .map(|asset| asset.base.id)
    .collect();
    assert_eq!(ret_group, vec![asset_id]);
    assert_eq!(ret_group2, vec![]);
    assert_ok!(repository::timeline_group::add_assets_to_group(
        &mut conn,
        group2_id,
        &[asset2_id, asset3_id]
    ));
    let ret_group2: Vec<AssetId> = assert_ok!(repository::timeline_group::get_assets_in_group(
        &mut conn, group2_id
    ))
    .into_iter()
    .map(|asset| asset.base.id)
    .collect();
    assert_eq!(ret_group2, vec![asset2_id, asset3_id]);

    // Remove assets from wrong group
    let _ = assert_err!(repository::timeline_group::remove_assets_from_group(
        &mut conn,
        group2_id,
        &[asset_id]
    ));
    let _ = assert_err!(repository::timeline_group::remove_assets_from_group(
        &mut conn,
        group2_id,
        &[asset_id, asset3_id]
    ));
    let _ = assert_err!(repository::timeline_group::remove_assets_from_group(
        &mut conn,
        group_id,
        &[asset4_id]
    ));

    // Correctly remove from group, now empty.
    assert_ok!(repository::timeline_group::remove_assets_from_group(
        &mut conn,
        group_id,
        &[asset_id]
    ));
    let ret_group: Vec<AssetId> = assert_ok!(repository::timeline_group::get_assets_in_group(
        &mut conn, group_id
    ))
    .into_iter()
    .map(|asset| asset.base.id)
    .collect();
    assert_eq!(&ret_group, &[]);

    // Correctly remove from group
    assert_ok!(repository::timeline_group::remove_assets_from_group(
        &mut conn,
        group2_id,
        &[asset3_id]
    ));
    let ret_group2: Vec<AssetId> = assert_ok!(repository::timeline_group::get_assets_in_group(
        &mut conn, group2_id
    ))
    .into_iter()
    .map(|asset| asset.base.id)
    .collect();
    assert_eq!(&ret_group2, &[asset2_id]);
}
