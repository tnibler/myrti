use std::{collections::HashSet, ops::Not};

use camino::Utf8PathBuf as PathBuf;
use chrono::Months;
use claims::{assert_err, assert_ok};
use itertools::Itertools;
use pretty_assertions::assert_eq;
use proptest::prelude::*;

use proptest_arb::{arb_new_asset, arb_new_video_asset};

use crate::model::{
    CreateAssetBase,CreateAssetImage, CreateAssetVideo,CreateAssetSpe,
    repository, Asset, AssetBase, AssetId, AssetRootDir, AssetRootDirId, AssetSpe, AssetType,
    AudioRepresentation, AudioRepresentationId, CreateAsset, Image, Size, TimestampInfo, Video,
    VideoAsset, VideoRepresentation, VideoRepresentationId,
};

use super::*;

#[test]
fn prop_insert_retrieve_asset() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    proptest!(|(asset in arb_new_asset(AssetRootDirId(1)))| {
        rt.block_on(async {
            let pool = create_db().await;
            let asset_root_dir = AssetRootDir {
                id: AssetRootDirId(0),
                path: PathBuf::from("/path/to/assets"),
            };
            let root_dir_id = assert_ok!(
                repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await
            );
            let path_exists = repository::asset::asset_or_duplicate_with_path_exists(&pool, root_dir_id, &asset.base.file_path).await;
            prop_assert!(path_exists.is_ok());
            if path_exists.unwrap() {
                return Ok(());
            }
            let ffprobe_output: Option<&[u8]> = match &asset.sp {
                AssetSpe::Video(video) => Some(&[]),
                _ => None
            };
            #[allow(deprecated)]
            let insert_result = repository::asset::insert_asset(&pool, &asset, ffprobe_output).await;
            prop_assert!(insert_result.is_ok(), "insert failed: {:?}", insert_result.unwrap_err());
            let asset_id = insert_result.unwrap();
            let asset_with_id = Asset {
                base: AssetBase {
                    id: asset_id,
                    ..asset.base
                },
                ..asset
            };
            let retrieved = repository::asset::get_asset(&pool, asset_id).await;
            prop_assert!(retrieved.is_ok(), "retrieve failed: {:?}", retrieved.unwrap_err());
            prop_assert_eq!(asset_with_id, retrieved.unwrap());
            Ok(())
        })?
    });
}

#[tokio::test]
#[allow(unused_must_use, deprecated)]
async fn inserting_mismatching_asset_ty_and_spe_fails() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset = Asset {
        sp: AssetSpe::Image(Image {
            image_format_name: "jpeg".into(),
        }),
        base: AssetBase {
            id: AssetId(0),
            root_dir_id,
            ty: AssetType::Video,
            file_type: "jpeg".to_owned(),
            file_path: PathBuf::from("image.jpg"),
            is_hidden: false,
            added_at: utc_now_millis_zero(),
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
            thumb_small_square_avif: false,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: false,
            thumb_large_orig_size: None,
            thumb_small_square_size: None,
        }
    };
    // should fail both with and without ffprobe_output
    assert_err!(repository::asset::insert_asset(&pool, &asset, None::<&[u8]>).await);
    assert_err!(repository::asset::insert_asset(&pool, &asset, Some(&[])).await);

    let asset2 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: false,
        }),
        base: AssetBase {
            ty: AssetType::Image,
            ..asset.base
        }
    };
    assert_err!(repository::asset::insert_asset(&pool, &asset2, Some(&[])).await);
    assert_err!(repository::asset::insert_asset(&pool, &asset2, None::<&[u8]>).await);
}

#[test]
fn prop_get_assets_with_missing_thumbnails() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let pool = rt.block_on(async { create_db().await });
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id = assert_ok!(rt.block_on(async {
        repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await
    }));
    prop_compose! {
        fn arb_asset_with_some_thumbnails(
            asset_root_dir_id: AssetRootDirId,
        )
        (
            asset in arb_new_asset(asset_root_dir_id),
            thumb_present in any::<(bool, bool, bool, bool)>(),
        ) -> Asset {
            Asset {
                base: AssetBase {
                    thumb_small_square_avif: thumb_present.0,
                    thumb_small_square_webp: thumb_present.1,
                    thumb_large_orig_avif: thumb_present.2,
                    thumb_large_orig_webp: thumb_present.3,
                    thumb_small_square_size: if thumb_present.0 || thumb_present.1 {
                        Some(Size { width: 100, height: 100 })
                    } else {
                        None
                    },
                    thumb_large_orig_size:  if thumb_present.2 || thumb_present.3 {
                        Some(Size { width: 200, height: 300 })
                    } else {
                        None
                    },
                    ..asset.base
                },
                ..asset
            }
        }
    }
    proptest!(|(assets in prop::collection::vec(arb_asset_with_some_thumbnails(root_dir_id), 5..20))| {
        let _ = rt.block_on(async {
            let mut assets_with_ids: Vec<Asset> = Vec::default();
            for asset in assets {
                let ffprobe_output: Option<&[u8]> = match &asset.sp {
                    AssetSpe::Video(video) => Some(&[]),
                    _ => None
                };
                #[allow(deprecated)]
                let insert_result = repository::asset::insert_asset(&pool, &asset, ffprobe_output).await;
                prop_assert!(insert_result.is_ok());
                let asset_id = insert_result.unwrap();
                let asset_with_id = Asset {
                    base: AssetBase {
                        id: asset_id,
                        ..asset.base
                    },
                    ..asset
                };
                assets_with_ids.push(asset_with_id);
            }
            let expected_with_missing_thumb: HashSet<AssetId> = assets_with_ids.iter()
                .filter(|asset| {
                    !(asset.base.thumb_small_square_avif &&
                    asset.base.thumb_small_square_webp &&
                    asset.base.thumb_large_orig_avif &&
                    asset.base.thumb_large_orig_webp)
                })
                .map(|asset| asset.base.id)
                .collect();
            let actual = repository::asset::get_assets_with_missing_thumbnail(&pool, None).await;
            prop_assert!(actual.is_ok());
            let actual_ids: HashSet<AssetId> = actual.unwrap().iter().map(|asset| asset.id).collect();
            prop_assert_eq!(actual_ids, expected_with_missing_thumb);
            Ok(())
        });
    });
}

#[tokio::test]
async fn get_videos_without_dash() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/more/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let root_dir2_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir2).await);
    let asset = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("opus".to_owned()),
            has_dash: false,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id: root_dir2_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            is_hidden: false,
            added_at: utc_now_millis_zero(),
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
            thumb_small_square_avif: true,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: true,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
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
        rotation_correction: None,
        hash: None,
        gps_coordinates: None,
        }
    };
    let asset3 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("aac".into()),
            has_dash: true,
        }),
        base: AssetBase {
            root_dir_id: root_dir2_id,
            file_path: "/some/video.mp4".into(),
            ..asset.base.clone()
        },
    };
    let asset4 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("mp3".into()),
            has_dash: false,
        }),
        base: AssetBase {
            root_dir_id: root_dir2_id,
            file_path: "/some/video2.mp4".into(),
            ..asset.base.clone()
        },
    };
    let ffprobe_output1: Option<&[u8]> = Some(&[]);
    let ffprobe_output3: Option<&[u8]> = Some(&[]);
    let ffprobe_output4: Option<&[u8]> = Some(&[]);
    #[allow(deprecated)]
    let asset_id = assert_ok!(repository::asset::insert_asset(&pool, &asset, ffprobe_output1).await);
    let _asset2_id = assert_ok!(repository::asset::create_asset(&pool, asset2).await);
    #[allow(deprecated)]
    let _asset3_id = assert_ok!(repository::asset::insert_asset(&pool, &asset3, ffprobe_output3).await);
    #[allow(deprecated)]
    let asset4_id = assert_ok!(repository::asset::insert_asset(&pool, &asset4, ffprobe_output4).await);
    let videos_without_dash: HashSet<VideoAsset> =
        assert_ok!(repository::asset::get_video_assets_without_dash(&pool).await)
            .into_iter()
            .collect();
    let expected: HashSet<VideoAsset> = [
        Asset {
            base: AssetBase {
                id: asset_id,
                ..asset.base
            },
            ..asset
        },
        Asset {
            base: AssetBase {
                id: asset4_id,
                ..asset4.base
            },
            ..asset4
        },
    ]
    .into_iter()
    .map(|a| a.try_into().unwrap())
    .collect();
    assert_eq!(videos_without_dash, expected);
}

#[tokio::test]
async fn get_videos_in_acceptable_codec_without_dash() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    // h264 aac with dash
    let asset1 = Asset {
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            is_hidden: false,
            added_at: utc_now_millis_zero(),
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
            thumb_small_square_avif: true,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: true,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: true,
        }),
    };
    // h264 flac no dash
    let asset2 = Asset {
        base: AssetBase {
            file_path: "video2.mp4".into(),
            ..asset1.base.clone()
        },
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("flac".into()),
            has_dash: false,
        }),
    };
    // hevc aac with dash
    let asset3 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: true,
        }),
        base: AssetBase {
            file_path: "video3.mp4".into(),
            ..asset1.base.clone()
        },
    };
    // hevc aac no dash
    let asset4 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: false,
        }),
        base: AssetBase {
            file_path: "video4.mp4".into(),
            ..asset1.base.clone()
        },
    };
    // hevc mp3 no dash
    let asset5 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("mp3".into()),
            has_dash: false,
        }),
        base: AssetBase {
            file_path: "video5.mp4".into(),
            ..asset1.base.clone()
        },
    };
    #[allow(deprecated)]
    let _asset1_id = assert_ok!(repository::asset::insert_asset(&pool, &asset1, Some(&[])).await);
    #[allow(deprecated)]
    let asset2_id = assert_ok!(repository::asset::insert_asset(&pool, &asset2, Some(&[])).await);
    #[allow(deprecated)]
    let _asset3_id = assert_ok!(repository::asset::insert_asset(&pool, &asset3, Some(&[])).await);
    #[allow(deprecated)]
    let asset4_id = assert_ok!(repository::asset::insert_asset(&pool, &asset4, Some(&[])).await);
    #[allow(deprecated)]
    let asset5_id = assert_ok!(repository::asset::insert_asset(&pool, &asset5, Some(&[])).await);
    let acceptable_video_codecs1 = ["h264"];
    let acceptable_audio_codecs1 = ["aac", "flac"];
    let result1: HashSet<AssetId> = assert_ok!(
        repository::asset::get_videos_in_acceptable_codec_without_dash(
            &pool,
            acceptable_video_codecs1,
            acceptable_audio_codecs1
        )
        .await
    )
    .into_iter()
    .map(|a| a.base.id)
    .collect();
    let expected1: HashSet<AssetId> = [asset2_id].into_iter().collect();
    assert_eq!(result1, expected1);

    let acceptable_video_codecs2 = ["h264", "hevc"];
    let acceptable_audio_codecs2 = ["aac"];
    let result2: HashSet<AssetId> = assert_ok!(
        repository::asset::get_videos_in_acceptable_codec_without_dash(
            &pool,
            acceptable_video_codecs2,
            acceptable_audio_codecs2
        )
        .await
    )
    .into_iter()
    .map(|a| a.base.id)
    .collect();
    let expected2: HashSet<AssetId> = [asset4_id].into_iter().collect();
    assert_eq!(result2, expected2);

    let acceptable_video_codecs3 = ["h264", "hevc"];
    let acceptable_audio_codecs3 = ["aac", "mp3", "flac"];
    let result3: HashSet<AssetId> = assert_ok!(
        repository::asset::get_videos_in_acceptable_codec_without_dash(
            &pool,
            acceptable_video_codecs3,
            acceptable_audio_codecs3
        )
        .await
    )
    .into_iter()
    .map(|a| a.base.id)
    .collect();
    let expected3: HashSet<AssetId> = [asset2_id, asset4_id, asset5_id].into_iter().collect();
    assert_eq!(result3, expected3);
}

#[test]
fn prop_get_videos_with_no_acceptable_codec_repr() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    prop_compose! {
        fn arb_new_video_repr(
            codec_name: String
        )
        (
            width in 600_i64..4000,
            height in 600_i64..4000,
            bitrate in 80_000_i64..8_000_000,
            file_key in r".*\.mp4",
        ) -> VideoRepresentation {
            VideoRepresentation {
                id: VideoRepresentationId(0),
                asset_id: AssetId(0),
                codec_name: codec_name.clone(),
                bitrate,
                width,
                height,
                media_info_key: format!("{}.media_info", file_key),
                file_key,
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
                media_info_key: format!("{}.media_info", file_key),
                file_key,
            }
        }
    }
    prop_compose! {
        fn arb_video_asset_with_some_reprs(
            asset_root_dir_id: AssetRootDirId,
        )
        (
            asset in arb_new_video_asset(asset_root_dir_id),
            video_repr_codecs in prop::collection::vec("h264|hevc|av1|vp9", 0..3),
            audio_repr_codec in prop_oneof![
                1 => Just(None),
                3 => "mp3|aac|opus".prop_map(|codec| Some(codec))
            ],
        )
        (
            asset in Just(asset),
            video_reprs in video_repr_codecs.into_iter().map(|codec| arb_new_video_repr(codec)).collect::<Vec<_>>(),
            audio_repr in match audio_repr_codec {
                    None => Just(None).boxed(),
                    Some(codec) => arb_new_audio_repr(codec).prop_map(|r| Some(r)).boxed()
            }
        )-> (VideoAsset, Vec<VideoRepresentation>, Option<AudioRepresentation>) {
            (asset, video_reprs, audio_repr)
        }
    }
    proptest!(|(assets_and_reprs in prop::collection::vec(arb_video_asset_with_some_reprs(AssetRootDirId(1)), 0..20),
                acceptable_video_codecs in prop::collection::hash_set("h264|hevc|av1|vp9", 0..5),
                acceptable_audio_codecs in prop::collection::hash_set("mp3|aac|opus|flac", 0..5))| {
        let _ = rt.block_on(async {
            let pool = create_db().await;
            let asset_root_dir = AssetRootDir {
                id: AssetRootDirId(0),
                path: PathBuf::from("/path/to/assets"),
            };
            let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
            let mut assets_with_ids: Vec<VideoAsset> = Vec::default();
            for (asset, video_reprs, audio_repr) in &assets_and_reprs {
                let path_exists = repository::asset::asset_or_duplicate_with_path_exists(&pool, root_dir_id, &asset.base.file_path).await;
                prop_assert!(path_exists.is_ok());
                if path_exists.unwrap() {
                    continue;
                }
                let ffprobe_output = Some(&[]);
                #[allow(deprecated)]
                let asset_insert_result = repository::asset::insert_asset(&pool, &asset.into(), ffprobe_output).await;
                prop_assert!(asset_insert_result.is_ok());
                let asset_id = asset_insert_result.unwrap();
                assets_with_ids.push(VideoAsset {
                    base:AssetBase {
                        id: asset_id,
                        ..asset.base.clone()
                    },
                    ..asset.clone()
                });
                let mut tx = pool.begin().await.unwrap();
                for repr in video_reprs {
                    let repr_insert_result = repository::representation::insert_video_representation(
                        tx.as_mut(), 
                        &VideoRepresentation {
                            asset_id: asset_id,
                            ..repr.clone()
                    }).await;
                    prop_assert!(repr_insert_result.is_ok());
                }
                if let Some(repr) = audio_repr {
                    let repr_insert_result = repository::representation::insert_audio_representation(
                        tx.as_mut(), 
                        &AudioRepresentation {
                            asset_id: asset_id,
                            ..repr.clone()
                    }).await;
                    prop_assert!(repr_insert_result.is_ok());
                }
                prop_assert!(tx.commit().await.is_ok());
            }
            let expected_no_acceptable_reprs: HashSet<AssetId> = assets_with_ids.iter().zip(assets_and_reprs.iter())
                .filter(|(asset, (_, video_reprs, audio_repr))| {
                    let mut video_repr_codecs: HashSet<String> = video_reprs.iter().map(|repr| repr.codec_name.clone()).collect::<HashSet<_>>();
                    if asset.base.file_type == "mp4" {
                        video_repr_codecs.insert(asset.video.video_codec_name.clone());
                    }
                    let video_repr_missing = acceptable_video_codecs.intersection(&video_repr_codecs).collect_vec().is_empty();
                    let audio_repr_missing = match &asset.video.audio_codec_name {
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
                &pool,
                acceptable_video_codecs.iter().map(|s| s.as_ref()),
                acceptable_audio_codecs.iter().map(|s| s.as_ref()),
            )
                .await;
            prop_assert!(actual.is_ok(), "retrieving failed: {:?}", actual.unwrap_err());
            let actual_ids: HashSet<AssetId> = actual.unwrap().iter().map(|asset| asset.base.id).collect();
            prop_assert_eq!(actual_ids, expected_no_acceptable_reprs);
            Ok(())
        })?;
    });
}
