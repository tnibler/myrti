use std::path::PathBuf;

use chrono::{Months, Utc};
use claims::{assert_err, assert_ok};

use crate::model::{
    repository, Asset, AssetBase, AssetId, AssetRootDir, AssetRootDirId, AssetSpe, AssetType,
    Image, MediaTimestamp, Size, Video,
};

use super::*;

#[tokio::test]
async fn insert_retrieve_asset() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id = repository::asset_root_dir::insert_asset_root(&pool, asset_root_dir).await;
    assert_ok!(root_dir_id);
    let asset = Asset {
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Image,
            root_dir_id: root_dir_id.unwrap(),
            file_path: PathBuf::from("image.jpg"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(2)).unwrap()),
            size: Size {
                width: 1024,
                height: 1024,
            },
            rotation_correction: None,
            hash: None,
            thumb_small_square_avif: None,
            thumb_small_square_webp: None,
            thumb_large_orig_avif: None,
            thumb_large_orig_webp: None,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset_id = assert_ok!(repository::asset::insert_asset(&pool, &asset).await);
    let retrieved = assert_ok!(repository::asset::get_asset(&pool, asset_id).await);
    let mut orig_with_id = asset.clone();
    orig_with_id.base.id = retrieved.base.id;
    assert_eq!(retrieved, orig_with_id);
    let all_assets = assert_ok!(repository::asset::get_assets(&pool).await);
    assert_eq!(all_assets, [orig_with_id]);
}

#[tokio::test]
async fn insert_mismatching_asset_ty_and_spe_fails() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, asset_root_dir).await);
    let asset = Asset {
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id,
            file_path: PathBuf::from("image.jpg"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(2)).unwrap()),
            size: Size {
                width: 1024,
                height: 1024,
            },
            rotation_correction: None,
            hash: None,
            thumb_small_square_avif: None,
            thumb_small_square_webp: None,
            thumb_large_orig_avif: None,
            thumb_large_orig_webp: None,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    assert_err!(repository::asset::insert_asset(&pool, &asset).await);

    let asset2 = Asset {
        sp: AssetSpe::Video(Video {
            codec_name: String::from("h264"),
            dash_resource_dir: None,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Image,
            ..asset.base.clone()
        },
    };
    assert_err!(repository::asset::insert_asset(&pool, &asset2).await);
}
