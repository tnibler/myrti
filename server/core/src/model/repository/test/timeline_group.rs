use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf as PathBuf;
use chrono::{Months, Utc};
use claims::{assert_err, assert_ok};

use crate::model::{
    repository::{self, album::CreateAlbum, timeline_group::CreateTimelineGroup},
    Album, Asset, AssetId, AssetRootDir, AssetRootDirId, CreateAsset, CreateAssetBase,
    CreateAssetImage, CreateAssetSpe, Size, TimestampInfo,
};

use super::*;

#[tokio::test]
#[allow(unused_must_use)]
async fn adding_asset_to_multiple_group_albums_fails() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
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
    let asset_id = assert_ok!(repository::asset::create_asset(&pool, asset.clone()).await);
    let asset2_id = assert_ok!(repository::asset::create_asset(&pool, asset2.clone()).await);
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
    let group_id =
        assert_ok!(repository::timeline_group::create_timeline_group(&pool, group).await);
    let group2_id =
        assert_ok!(repository::timeline_group::create_timeline_group(&pool, group2).await);
    assert_ok!(
        repository::timeline_group::add_assets_to_group(
            pool.acquire().await.unwrap().as_mut(),
            group_id,
            &[asset_id]
        )
        .await
    );
    assert_err!(
        repository::timeline_group::add_assets_to_group(
            pool.acquire().await.unwrap().as_mut(),
            group2_id,
            &[asset_id]
        )
        .await
    );
    let ret_group: Vec<AssetId> =
        assert_ok!(repository::timeline_group::get_assets_in_group(&pool, group_id).await)
            .into_iter()
            .map(|asset| asset.base.id)
            .collect();
    let ret_group2: Vec<AssetId> =
        assert_ok!(repository::timeline_group::get_assets_in_group(&pool, group2_id).await)
            .into_iter()
            .map(|asset| asset.base.id)
            .collect();
    assert_eq!(ret_group, vec![asset_id]);
    assert_eq!(ret_group2, vec![]);
    assert_ok!(
        repository::timeline_group::add_assets_to_group(
            pool.acquire().await.unwrap().as_mut(),
            group_id,
            &[asset2_id]
        )
        .await
    );
}
