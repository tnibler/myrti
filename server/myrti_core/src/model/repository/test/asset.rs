use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use chrono::Months;
use claims::assert_ok;
use diesel::prelude::*;
use itertools::Itertools;
use pretty_assertions::assert_eq;
use proptest::prelude::*;

use proptest_arb::{arb_new_asset, arb_new_video_asset};

use crate::model::repository::test::util::{make_created_asset, prop_insert_create_test_asset};
use crate::model::{
    repository, Asset, AssetId, AssetRootDir, AssetRootDirId, AssetSpe, AssetThumbnail,
    AssetThumbnailId, AudioRepresentation, AudioRepresentationId, CreateAsset, CreateAssetBase,
    CreateAssetImage, CreateAssetSpe, CreateAssetVideo, CreateAudioRepresentation,
    CreateVideoRepresentation, FFProbeOutput, Size, ThumbnailFormat, ThumbnailType, TimestampInfo,
    VideoAsset, VideoRepresentation, VideoRepresentationId,
};

use super::util::set_asset_root_dir;
use super::*;

#[test]
fn prop_insert_retrieve_asset() {
    proptest!(|(asset in arb_new_asset())| {
        let mut conn = super::db::open_in_memory_and_migrate();
        let asset_root_dir = AssetRootDir {
            id: AssetRootDirId(0),
            path: PathBuf::from("/path/to/assets"),
        };
        let root_dir_id = assert_ok!(
            repository::asset_root_dir::insert_asset_root(&mut conn, &asset_root_dir)
        );
        let asset = set_asset_root_dir(asset, root_dir_id);
        let path_exists = repository::asset::asset_or_duplicate_with_path_exists(&mut conn, root_dir_id, &asset.base.file_path);
        prop_assert!(path_exists.is_ok());
        if path_exists.unwrap() {
            return Ok(());
        }
        let insert_result = repository::asset::create_asset(&mut conn, asset.clone());
        prop_assert!(insert_result.is_ok(), "insert failed: {:?}", insert_result.unwrap_err());
        let asset_id = insert_result.unwrap();

        let retrieved = repository::asset::get_asset(&mut conn, asset_id);
        prop_assert!(retrieved.is_ok(), "retrieve failed: {:?}", retrieved.unwrap_err());
        let retrieved = retrieved.unwrap();

        let asset_with_id = make_created_asset(&asset, &retrieved);
        prop_assert_eq!(asset_with_id, retrieved);
    });
}

#[test]
fn prop_get_assets_with_missing_thumbnails() {
    prop_compose! {
        fn arb_asset_with_some_thumbnails()(
            asset in arb_new_asset(),
            thumb_present in any::<(bool, bool)>(),
        ) -> (CreateAsset, bool, bool) {
            (asset, thumb_present.0, thumb_present.1)
        }
    }
    proptest!(|(assets_thumb_present in prop::collection::vec(arb_asset_with_some_thumbnails(), 1..20))| {
        let mut conn = super::db::open_in_memory_and_migrate();
        let asset_root_dir = AssetRootDir {
            id: AssetRootDirId(0),
            path: PathBuf::from("/path/to/assets"),
        };
        let root_dir_id = assert_ok!(
            repository::asset_root_dir::insert_asset_root(&mut conn, &asset_root_dir)
        );
        let assets_thumb_present: Vec<_> = assets_thumb_present
            .into_iter()
            .map(|(asset, t_lg_orig, t_sm_sq)|
                (set_asset_root_dir(asset, root_dir_id), t_lg_orig, t_sm_sq)
            ).collect();
        let mut assets_with_ids: Vec<(AssetId, bool, bool)> = Vec::default();
        for (asset, has_lg_orig, has_sm_sq) in assets_thumb_present {
            let insert_result = repository::asset::create_asset(&mut conn, asset);
            prop_assert!(insert_result.is_ok());
            let asset_id = insert_result.unwrap();
            if has_lg_orig {
                assert_ok!(repository::asset::insert_asset_thumbnail(&mut conn, AssetThumbnail {
                    id: AssetThumbnailId(0),
                    asset_id,
                    ty: ThumbnailType::LargeOrigAspect,
                    size: Size { width: 100, height: 100 },
                    format: ThumbnailFormat::Avif
                }));
                assert_ok!(repository::asset::insert_asset_thumbnail(&mut conn, AssetThumbnail {
                    id: AssetThumbnailId(0),
                    asset_id,
                    ty: ThumbnailType::LargeOrigAspect,
                    size: Size { width: 100, height: 100 },
                    format: ThumbnailFormat::Webp
                }));
            }
            if has_sm_sq {
                assert_ok!(repository::asset::insert_asset_thumbnail(&mut conn, AssetThumbnail {
                    id: AssetThumbnailId(0),
                    asset_id,
                    ty: ThumbnailType::SmallSquare,
                    size: Size { width: 100, height: 100 },
                    format: ThumbnailFormat::Avif
                }));
                assert_ok!(repository::asset::insert_asset_thumbnail(&mut conn, AssetThumbnail {
                    id: AssetThumbnailId(0),
                    asset_id,
                    ty: ThumbnailType::SmallSquare,
                    size: Size { width: 100, height: 100 },
                    format: ThumbnailFormat::Webp
                }));
            }
            assets_with_ids.push((asset_id, has_lg_orig, has_sm_sq));
        }
        let expected_with_missing_thumb: HashSet<AssetId> = assets_with_ids.into_iter()
            .filter(|(_asset_id, has_lg_orig, has_sm_sq)| {
                !(*has_lg_orig && *has_sm_sq)
            })
            .map(|(asset_id, _, _)| asset_id)
            .collect();
        let actual = repository::asset::get_assets_with_missing_thumbnail(&mut conn, None);
        prop_assert!(actual.is_ok());
        let actual_ids: HashSet<AssetId> = actual.unwrap().iter().map(|asset| asset.asset_id).collect();
        prop_assert_eq!(actual_ids, expected_with_missing_thumb);
    });
}

#[test]
fn get_videos_without_dash() {
    let mut conn = super::db::open_in_memory_and_migrate();
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/more/assets"),
    };
    let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir
    ));
    let root_dir2_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir2
    ));
    let asset = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("opus".to_owned()),
            has_dash: false,
            video_duration_ms: None,
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
        base: CreateAssetBase {
            root_dir_id: root_dir2_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            is_hidden: false,
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(3))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 100,
                height: 100,
            },
            rotation_correction: Some(90),
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
            file_path: "/path/to/image.jpg".into(),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(4))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1000,
                height: 1000,
            },
            is_hidden: false,
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset3 = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("aac".into()),
            has_dash: true,
            video_duration_ms: None,
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
        base: CreateAssetBase {
            root_dir_id: root_dir2_id,
            file_path: "/some/video.mp4".into(),
            ..asset.base.clone()
        },
    };
    let asset4 = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("mp3".into()),
            has_dash: false,
            video_duration_ms: Some(123433323),
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
        base: CreateAssetBase {
            root_dir_id: root_dir2_id,
            file_path: "/some/video2.mp4".into(),
            ..asset.base.clone()
        },
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&mut conn, asset.clone()));
    let _asset2_id = assert_ok!(repository::asset::create_asset(&mut conn, asset2));
    let _asset3_id = assert_ok!(repository::asset::create_asset(&mut conn, asset3,));
    let asset4_id = assert_ok!(repository::asset::create_asset(&mut conn, asset4.clone()));
    let videos_without_dash: HashSet<VideoAsset> =
        assert_ok!(repository::asset::get_video_assets_without_dash(&mut conn))
            .into_iter()
            .collect();
    let expected: HashSet<VideoAsset> = [
        make_created_asset(
            &asset4,
            &assert_ok!(repository::asset::get_asset(&mut conn, asset4_id)),
        ),
        make_created_asset(
            &asset,
            &assert_ok!(repository::asset::get_asset(&mut conn, asset_id)),
        ),
    ]
    .into_iter()
    .map(|a| a.try_into().unwrap())
    .collect();
    assert_eq!(
        videos_without_dash, expected,
        "\n{:?}\n{:?}",
        expected, videos_without_dash
    );
}

#[test]
fn get_videos_in_acceptable_codec_without_dash() {
    let mut conn = super::db::open_in_memory_and_migrate();
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir
    ));
    // h264 aac with dash
    let asset1 = CreateAsset {
        base: CreateAssetBase {
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            is_hidden: false,
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(3))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 100,
                height: 100,
            },
            rotation_correction: Some(90),
            hash: None,
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: true,
            video_duration_ms: Some(43444444),
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
    };
    // h264 flac no dash
    let asset2 = CreateAsset {
        base: CreateAssetBase {
            file_path: "video2.mp4".into(),
            ..asset1.base.clone()
        },
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("flac".into()),
            has_dash: false,
            video_duration_ms: None,
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
    };
    // hevc aac with dash
    let asset3 = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "hevc".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: true,
            video_duration_ms: None,
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
        base: CreateAssetBase {
            file_path: "video3.mp4".into(),
            ..asset1.base.clone()
        },
    };
    // hevc aac no dash
    let asset4 = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "hevc".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: false,
            video_duration_ms: Some(12333),
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
        base: CreateAssetBase {
            file_path: "video4.mp4".into(),
            ..asset1.base.clone()
        },
    };
    // hevc mp3 no dash
    let asset5 = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "hevc".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("mp3".into()),
            has_dash: false,
            video_duration_ms: Some(1233),
            ffprobe_output: FFProbeOutput(Default::default()),
        }),
        base: CreateAssetBase {
            file_path: "video5.mp4".into(),
            ..asset1.base.clone()
        },
    };
    let _asset1_id = assert_ok!(repository::asset::create_asset(&mut conn, asset1,));
    let asset2_id = assert_ok!(repository::asset::create_asset(&mut conn, asset2,));
    let _asset3_id = assert_ok!(repository::asset::create_asset(&mut conn, asset3,));
    let asset4_id = assert_ok!(repository::asset::create_asset(&mut conn, asset4,));
    let asset5_id = assert_ok!(repository::asset::create_asset(&mut conn, asset5,));
    let acceptable_video_codecs1 = ["h264"];
    let acceptable_audio_codecs1 = ["aac", "flac"];
    repository::config::set_acceptable_video_codecs(&mut conn, acceptable_video_codecs1).unwrap();
    repository::config::set_acceptable_audio_codecs(&mut conn, acceptable_audio_codecs1).unwrap();
    let result1: HashSet<AssetId> =
        assert_ok!(repository::asset::get_videos_in_acceptable_codec_without_dash(&mut conn,))
            .into_iter()
            .map(|a| a.base.id)
            .collect();
    let expected1: HashSet<AssetId> = [asset2_id].into_iter().collect();
    assert_eq!(result1, expected1);

    let acceptable_video_codecs2 = ["h264", "hevc"];
    let acceptable_audio_codecs2 = ["aac"];
    repository::config::set_acceptable_video_codecs(&mut conn, acceptable_video_codecs2).unwrap();
    repository::config::set_acceptable_audio_codecs(&mut conn, acceptable_audio_codecs2).unwrap();
    let result2: HashSet<AssetId> =
        assert_ok!(repository::asset::get_videos_in_acceptable_codec_without_dash(&mut conn,))
            .into_iter()
            .map(|a| a.base.id)
            .collect();
    let expected2: HashSet<AssetId> = [asset4_id].into_iter().collect();
    assert_eq!(result2, expected2);

    let acceptable_video_codecs3 = ["h264", "hevc"];
    let acceptable_audio_codecs3 = ["aac", "mp3", "flac"];
    repository::config::set_acceptable_video_codecs(&mut conn, acceptable_video_codecs3).unwrap();
    repository::config::set_acceptable_audio_codecs(&mut conn, acceptable_audio_codecs3).unwrap();
    let result3: HashSet<AssetId> =
        assert_ok!(repository::asset::get_videos_in_acceptable_codec_without_dash(&mut conn,))
            .into_iter()
            .map(|a| a.base.id)
            .collect();
    let expected3: HashSet<AssetId> = [asset2_id, asset4_id, asset5_id].into_iter().collect();
    assert_eq!(result3, expected3);
}

#[test]
fn prop_get_videos_with_no_acceptable_codec_repr() {
    prop_compose! {
        fn arb_new_video_repr(
            codec_name: String
        )
        (
            width in 600_i32..4000,
            height in 600_i32..4000,
            bitrate in 80_000_i64..8_000_000,
            file_key in r".*\.mp4",
        ) -> VideoRepresentation {
            VideoRepresentation {
                id: VideoRepresentationId(0),
                asset_id: AssetId(0),
                name: format!("{}x{}", width, height),
                codec_name: codec_name.clone(),
                bitrate,
                width,
                height,
            }
        }
    }
    prop_compose! {
        fn arb_new_audio_repr(
            codec_name: String
        )
        (
            file_key in r".*_audio\.m4"
        ) -> AudioRepresentation {
            AudioRepresentation {
                id: AudioRepresentationId(0),
                asset_id: AssetId(0),
                codec_name: codec_name.clone(),
                name: codec_name.clone(),
            }
        }
    }
    prop_compose! {
        fn arb_video_asset_with_some_reprs()
        (
            asset in arb_new_video_asset(),
            video_repr_codecs in prop::collection::vec("h264|hevc|av1|vp9", 0..3),
            audio_repr_codec in prop_oneof![
                1 => Just(None),
                3 => "mp3|aac|opus".prop_map(Some)
            ],
        )
        (
            asset in Just(asset),
            video_reprs in video_repr_codecs.into_iter().map(arb_new_video_repr).collect::<Vec<_>>(),
            audio_repr in match audio_repr_codec {
                    None => Just(None).boxed(),
                    Some(codec) => arb_new_audio_repr(codec).prop_map(Some).boxed()
            }
        )-> (CreateAsset, Vec<VideoRepresentation>, Option<AudioRepresentation>) {
            let video = match &asset.spe {
                CreateAssetSpe::Video(video) => video,
                CreateAssetSpe::Image(_) => panic!(),
            };
            let audio_repr = match (&video.audio_codec_name, audio_repr) {
                // if the original asset has no audio we don't generate
                // additional audio reprs
                (None, _) => None,
                (_, ar) => ar,
            };
            (asset, video_reprs, audio_repr)
        }
    }
    proptest!(|(assets_and_reprs in prop::collection::vec(arb_video_asset_with_some_reprs(), 0..20),
                acceptable_video_codecs in prop::collection::hash_set("h264|hevc|av1|vp9", 0..5),
                acceptable_audio_codecs in prop::collection::hash_set("mp3|aac|opus|flac", 0..5))| {
        let mut conn = super::db::open_in_memory_and_migrate();
        let asset_root_dir = AssetRootDir {
            id: AssetRootDirId(0),
            path: PathBuf::from("/path/to/assets"),
        };
        let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(&mut conn, &asset_root_dir));
        repository::config::set_acceptable_video_codecs(&mut conn, &acceptable_video_codecs).unwrap();
        repository::config::set_acceptable_audio_codecs(&mut conn, &acceptable_audio_codecs).unwrap();
        let mut assets_with_ids: Vec<Asset> = Vec::default();
        let assets_and_reprs: Vec<_> = assets_and_reprs
            .into_iter()
            .map(|(asset, video_repr, audio_repr)|
                (set_asset_root_dir(asset, root_dir_id), video_repr, audio_repr)
        ).collect();
        for (asset, video_reprs, audio_repr) in &assets_and_reprs {
            let path_exists = repository::asset::asset_or_duplicate_with_path_exists(&mut conn, root_dir_id, &asset.base.file_path);
            prop_assert!(path_exists.is_ok());
            if path_exists.unwrap() {
                continue;
            }
            let asset = prop_insert_create_test_asset(&mut conn, asset);
            prop_assert!(asset.is_ok(), "Error inserting asset: {}", asset.unwrap_err());
            let asset = asset.unwrap();
            let asset_id = asset.base.id;
            assets_with_ids.push(asset);
            let tx_result = conn.transaction(|conn| {
                for repr in video_reprs {
                    let repr_id = repository::representation::insert_video_representation(
                        conn,
                        &CreateVideoRepresentation {
                            asset_id,
                            name: repr.name.clone(),
                            codec_name: repr.codec_name.clone()
                    });
                    prop_assert!(repr_id.is_ok());
                    let repr_id = repr_id.unwrap();
                    let finalize_result = repository::representation::finalize_video_representation(
                        conn,
                        &VideoRepresentation {
                            asset_id,
                            id: repr_id,
                            ..repr.clone()
                    });
                    prop_assert!(finalize_result.is_ok());
                }
                if let Some(repr) = audio_repr {
                    let repr_id = repository::representation::insert_audio_representation(
                        conn,
                        &CreateAudioRepresentation {
                            asset_id,
                            name: repr.name.clone(),
                            codec_name: repr.codec_name.clone()
                    });
                    prop_assert!(repr_id.is_ok());
                    let repr_id = repr_id.unwrap();
                    let finalize_result = repository::representation::finalize_audio_representation(
                        conn,
                        repr_id,
                    );
                    prop_assert!(finalize_result.is_ok());
                }
                Ok(())
            });
            prop_assert!(tx_result.is_ok());
        }
        let expected_no_acceptable_reprs: HashSet<AssetId> = assets_with_ids.iter().zip(assets_and_reprs.iter())
            .filter(|(asset, (_, video_reprs, audio_repr))| {
                let mut video_repr_codecs: HashSet<String> = video_reprs.iter().map(|repr| repr.codec_name.clone()).collect::<HashSet<_>>();
                let video = match &asset.sp {
                    AssetSpe::Video(video) => video,
                    AssetSpe::Image(_) => panic!(),
                };
                if asset.base.file_type == "mp4" {
                    video_repr_codecs.insert(video.video_codec_name.clone());
                }
                let video_repr_missing = acceptable_video_codecs.intersection(&video_repr_codecs).collect_vec().is_empty();
                let audio_repr_missing = match &video.audio_codec_name {
                    None => false,
                    Some(orig_codec) => {
                        let mut audio_repr_codecs: HashSet<String> = audio_repr.iter().map(|repr| repr.codec_name.clone()).collect();
                        audio_repr_codecs.insert(orig_codec.clone());
                        audio_repr_codecs.intersection(&acceptable_audio_codecs).collect_vec().is_empty()
                    }
                };
                video_repr_missing || audio_repr_missing
        })
            .map(|(asset, (_, _video_reprs, _audio_repr))| asset.base.id)
            .collect();
        let actual = repository::asset::get_video_assets_with_no_acceptable_repr(
            &mut conn,
        );
        prop_assert!(actual.is_ok(), "retrieving failed: {:?}", actual.unwrap_err());
        let actual_ids: HashSet<AssetId> = actual.unwrap().iter().map(|asset| asset.base.id).collect();
        prop_assert_eq!(actual_ids, expected_no_acceptable_reprs);
    });
}
