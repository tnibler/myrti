use camino::Utf8PathBuf as PathBuf;
use chrono::Months;
use claims::{assert_err, assert_ok};

use crate::model::{
    repository::{self, timeline_group::CreateTimelineGroup}, AssetId, AssetRootDir, AssetRootDirId, CreateAsset, CreateAssetBase,
    CreateAssetImage, CreateAssetSpe, Size, TimestampInfo,
};

use super::*;

#[test]
fn adding_asset_to_multiple_group_albums_fails() {
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
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
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
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
        },
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&mut conn, asset.clone()));
    let asset2_id = assert_ok!(repository::asset::create_asset(&mut conn, asset2.clone()));
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
        group_id,
        &[asset2_id]
    ));
}
