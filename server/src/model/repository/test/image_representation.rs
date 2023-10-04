use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use chrono::Months;
use claims::assert_ok;

use crate::model::{
    repository::{
        self,
        test::{create_db, utc_now_millis_zero},
    },
    AssetId, AssetRootDir, AssetRootDirId, AssetSpe, AssetType, CreateAsset, Image,
    ImageRepresentation, ImageRepresentationId, Size, TimestampInfo,
};

#[tokio::test]
async fn insert_retrieve_image_representation() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset = CreateAsset {
        sp: AssetSpe::Image(Image {
            image_format_name: "jpeg".into(),
        }),
        ty: AssetType::Image,
        root_dir_id,
        file_type: "jpeg".to_owned(),
        file_path: PathBuf::from("image.jpg"),
        taken_date: utc_now_millis_zero()
            .checked_sub_months(Months::new(2))
            .unwrap(),
        timestamp_info: TimestampInfo::UtcCertain,
        size: Size {
            width: 1024,
            height: 1023,
        },
        rotation_correction: None,
        hash: Some(0x56a28ebc104e84),
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&pool, asset).await);
    let image_reprs =
        assert_ok!(repository::representation::get_image_representations(&pool, asset_id).await);
    assert!(image_reprs.is_empty());
    let repr1 = ImageRepresentation {
        id: ImageRepresentationId(0),
        asset_id,
        format_name: "avif".into(),
        width: 1024,
        height: 1023,
        file_size: 123123,
        file_key: "img/some_key".into(),
    };
    let repr1_id =
        assert_ok!(repository::representation::insert_image_representation(&pool, &repr1).await);
    let repr1_with_id = ImageRepresentation {
        id: repr1_id,
        ..repr1
    };
    let expected = vec![repr1_with_id];
    let retrieved =
        assert_ok!(repository::representation::get_image_representations(&pool, asset_id).await);
    assert_eq!(expected, retrieved);
}

#[tokio::test]
async fn get_images_with_no_acceptable_repr() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset1 = CreateAsset {
        sp: AssetSpe::Image(Image {
            image_format_name: "jpeg".into(),
        }),
        ty: AssetType::Image,
        root_dir_id,
        file_type: "jpeg".to_owned(),
        file_path: PathBuf::from("image.jpg"),
        taken_date: utc_now_millis_zero()
            .checked_sub_months(Months::new(2))
            .unwrap(),
        timestamp_info: TimestampInfo::UtcCertain,
        size: Size {
            width: 1024,
            height: 1023,
        },
        rotation_correction: None,
        hash: Some(0x56a28ebc104e84),
    };
    let asset1_id = assert_ok!(repository::asset::create_asset(&pool, asset1).await);
    let asset2 = CreateAsset {
        sp: AssetSpe::Image(Image {
            image_format_name: "heif".into(),
        }),
        ty: AssetType::Image,
        root_dir_id,
        file_type: "heif".to_owned(),
        file_path: PathBuf::from("image.heif"),
        taken_date: utc_now_millis_zero()
            .checked_sub_months(Months::new(2))
            .unwrap(),
        timestamp_info: TimestampInfo::UtcCertain,
        size: Size {
            width: 1024,
            height: 1023,
        },
        rotation_correction: None,
        hash: Some(0x123),
    };
    let asset2_id = assert_ok!(repository::asset::create_asset(&pool, asset2).await);

    let acceptable_formats = ["jpeg"].into_iter();
    let actual: HashSet<AssetId> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&pool, acceptable_formats)
            .await
    )
    .into_iter()
    .collect();
    let expected: HashSet<AssetId> = [asset2_id].into_iter().collect();
    assert_eq!(expected, actual);

    let acceptable_formats = ["jpeg", "heif"].into_iter();
    let actual: HashSet<AssetId> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&pool, acceptable_formats)
            .await
    )
    .into_iter()
    .collect();
    let expected: HashSet<AssetId> = [].into_iter().collect();
    assert_eq!(expected, actual);

    let asset2_repr = ImageRepresentation {
        id: ImageRepresentationId(0),
        asset_id: asset2_id,
        format_name: "avif".into(),
        width: 100,
        height: 100,
        file_size: 94949494,
        file_key: "img/key2".into(),
    };
    assert_ok!(repository::representation::insert_image_representation(&pool, &asset2_repr).await);

    let acceptable_formats = ["jpeg"].into_iter();
    let actual: HashSet<AssetId> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&pool, acceptable_formats)
            .await
    )
    .into_iter()
    .collect();
    let expected: HashSet<AssetId> = [asset2_id].into_iter().collect();
    assert_eq!(expected, actual);

    let acceptable_formats = ["jpeg", "avif"].into_iter();
    let actual: HashSet<AssetId> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&pool, acceptable_formats)
            .await
    )
    .into_iter()
    .collect();
    let expected: HashSet<AssetId> = [].into_iter().collect();
    assert_eq!(expected, actual);
}
