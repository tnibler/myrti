use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use chrono::Months;
use claims::assert_ok;

use crate::model::{
    AssetId, AssetRootDir, AssetRootDirId, AssetSpe, CreateAsset, CreateAssetBase,
    CreateAssetImage, CreateAssetSpe, ImageFileId, ImageRepresentation, ImageRepresentationId,
    Size, TimestampInfo,
    repository::{self, test::utc_now_millis_zero},
};

#[test]
fn insert_retrieve_image_representation() {
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
                height: 1023,
            },
            is_hidden: false,
            rotation_correction: None,
            hash: Some(0x56a28ebc104e84),
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&mut conn, asset));
    let asset = assert_ok!(repository::asset::get_asset(&mut conn, asset_id));
    let asset_image = match asset.sp {
        AssetSpe::Image(image) => image,
        AssetSpe::Video(_) => panic!(),
    };
    let image_reprs = assert_ok!(repository::representation::get_image_representations(
        &mut conn,
        asset_image.image_file_id
    ));
    assert!(image_reprs.is_empty());
    let repr1 = ImageRepresentation {
        id: ImageRepresentationId(0),
        image_file_id: asset_image.image_file_id,
        format_name: "avif".into(),
        width: 1024,
        height: 1023,
        file_size: 123123,
        file_key: "img/some_key".into(),
    };
    let repr1_id = assert_ok!(repository::representation::insert_image_representation(
        &mut conn, &repr1
    ));
    let repr1_with_id = ImageRepresentation {
        id: repr1_id,
        ..repr1
    };
    let expected = vec![repr1_with_id];
    let retrieved = assert_ok!(repository::representation::get_image_representations(
        &mut conn,
        asset_image.image_file_id
    ));
    assert_eq!(expected, retrieved);
}

#[test]
fn get_images_with_no_acceptable_repr() {
    let mut conn = super::db::open_in_memory_and_migrate();
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir
    ));
    let asset1 = CreateAsset {
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
                height: 1023,
            },
            is_hidden: false,
            rotation_correction: None,
            hash: Some(0x56a28ebc104e84),
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let _asset1_id = assert_ok!(repository::asset::create_asset(&mut conn, asset1));
    let asset2 = CreateAsset {
        spe: CreateAssetSpe::Image(CreateAssetImage {
            image_format_name: "heif".into(),
        }),
        base: CreateAssetBase {
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
            is_hidden: false,
            rotation_correction: None,
            hash: Some(0x123),
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset2_id = assert_ok!(repository::asset::create_asset(&mut conn, asset2));
    let asset2 = assert_ok!(repository::asset::get_asset(&mut conn, asset2_id));
    let asset2_image = match asset2.sp {
        AssetSpe::Image(image) => image,
        AssetSpe::Video(_) => panic!(),
    };

    let acceptable_formats = ["jpeg"];
    let actual: HashSet<(ImageFileId, AssetId)> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&mut conn, &acceptable_formats)
    )
    .into_iter()
    .collect();
    let expected: HashSet<(ImageFileId, AssetId)> = [(asset2_image.image_file_id, asset2_id)]
        .into_iter()
        .collect();
    assert_eq!(expected, actual);

    let acceptable_formats = ["jpeg", "heif"];
    let actual: HashSet<(ImageFileId, AssetId)> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&mut conn, &acceptable_formats)
    )
    .into_iter()
    .collect();
    let expected: HashSet<(ImageFileId, AssetId)> = [].into_iter().collect();
    assert_eq!(expected, actual);

    let asset2_repr = ImageRepresentation {
        id: ImageRepresentationId(0),
        image_file_id: asset2_image.image_file_id,
        format_name: "avif".into(),
        width: 100,
        height: 100,
        file_size: 94949494,
        file_key: "img/key2".into(),
    };
    assert_ok!(repository::representation::insert_image_representation(
        &mut conn,
        &asset2_repr
    ));

    let acceptable_formats = ["jpeg"];
    let actual: HashSet<(ImageFileId, AssetId)> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&mut conn, &acceptable_formats)
    )
    .into_iter()
    .collect();
    let expected: HashSet<(ImageFileId, AssetId)> = [(asset2_image.image_file_id, asset2_id)]
        .into_iter()
        .collect();
    assert_eq!(expected, actual);

    let acceptable_formats = ["jpeg", "avif"];
    let actual: HashSet<(ImageFileId, AssetId)> = assert_ok!(
        repository::asset::get_image_assets_with_no_acceptable_repr(&mut conn, &acceptable_formats)
    )
    .into_iter()
    .collect();
    let expected: HashSet<(ImageFileId, AssetId)> = [].into_iter().collect();
    assert_eq!(expected, actual);
}
