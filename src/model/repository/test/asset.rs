use std::{collections::HashSet, path::PathBuf};

use chrono::{Months, Utc};
use claims::{assert_err, assert_ok};
use pretty_assertions::assert_eq;

use crate::model::{
    repository::{self, representation::insert_video_representation},
    Asset, AssetBase, AssetId, AssetRootDir, AssetRootDirId, AssetSpe, AssetType,
    AudioRepresentation, AudioRepresentationId, Image, MediaTimestamp, Size, Video, VideoAsset,
    VideoRepresentation, VideoRepresentationId,
};

use super::*;

#[tokio::test]
async fn insert_retrieve_asset() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id = repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await;
    assert_ok!(root_dir_id);
    let asset = Asset {
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Image,
            root_dir_id: root_dir_id.unwrap(),
            file_type: "jpeg".to_owned(),
            file_path: PathBuf::from("image.jpg"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(2)).unwrap()),
            size: Size {
                width: 1024,
                height: 1023,
            },
            rotation_correction: None,
            hash: Some(0x56a28ebc104e84),
            thumb_small_square_avif: Some("/path/1".into()),
            thumb_small_square_webp: Some("/path/2".into()),
            thumb_large_orig_avif: Some("/path/3".into()),
            thumb_large_orig_webp: Some("/path/4".into()),
            thumb_small_square_size: Some(Size {
                width: 50,
                height: 50,
            }),
            thumb_large_orig_size: Some(Size {
                width: 100,
                height: 100,
            }),
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
#[allow(unused_must_use)]
async fn insert_mismatching_asset_ty_and_spe_fails() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset = Asset {
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id,
            file_type: "jpeg".to_owned(),
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
            video_codec_name: "h264".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
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

#[tokio::test]
async fn insert_update_asset() {
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
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Image,
            root_dir_id,
            file_type: "jpeg".to_owned(),
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
    let asset2 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".into(),
            video_bitrate: 1234,
            audio_codec_name: Some("mp3".into()),
            dash_resource_dir: Some("/dash/dir".into()),
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id: root_dir2_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(3)).unwrap()),
            size: Size {
                width: 100,
                height: 100,
            },
            rotation_correction: Some(90),
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
    let asset2_id = assert_ok!(repository::asset::insert_asset(&pool, &asset2).await);
    let asset2_changed = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".into(),
            video_bitrate: 456,
            audio_codec_name: Some("aac".into()),
            dash_resource_dir: Some("/other/dir".into()),
        }),
        base: AssetBase {
            id: asset2_id,
            ty: asset2.base.ty,
            root_dir_id: asset2.base.root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("videoother.mp4"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(4)).unwrap()),
            size: Size {
                width: 101,
                height: 102,
            },
            rotation_correction: None,
            hash: Some(0x1234abcd1234),
            thumb_small_square_avif: Some("/path/1".into()),
            thumb_small_square_webp: Some("/path/2".into()),
            thumb_large_orig_avif: Some("/path/3".into()),
            thumb_large_orig_webp: Some("/path/4".into()),
            thumb_small_square_size: Some(Size {
                width: 12,
                height: 34,
            }),
            thumb_large_orig_size: Some(Size {
                width: 45,
                height: 67,
            }),
        },
    };
    assert_ok!(
        repository::asset::update_asset(pool.acquire().await.unwrap().as_mut(), &asset2_changed)
            .await
    );
    let retrieved = assert_ok!(repository::asset::get_asset(&pool, asset2_id).await);
    assert_eq!(retrieved, asset2_changed);
}

#[tokio::test]
async fn get_assets_with_missing_thumbnails() {
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
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Image,
            root_dir_id,
            file_type: "jpeg".to_owned(),
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
    let asset2 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            dash_resource_dir: Some("/dash/dir".into()),
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id: root_dir2_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(3)).unwrap()),
            size: Size {
                width: 100,
                height: 100,
            },
            rotation_correction: Some(90),
            hash: None,
            thumb_small_square_avif: Some("/path/1".into()),
            thumb_small_square_webp: None,
            thumb_large_orig_avif: None,
            thumb_large_orig_webp: Some("/path/2".into()),
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset3 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 456,
            audio_codec_name: Some("aac".into()),
            dash_resource_dir: Some("/dash/dir2".into()),
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id: root_dir2_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video3.mp4"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(3)).unwrap()),
            size: Size {
                width: 102,
                height: 104,
            },
            rotation_correction: Some(90),
            hash: None,
            thumb_small_square_avif: Some("/path/1".into()),
            thumb_small_square_webp: Some("/path/2".into()),
            thumb_large_orig_avif: Some("/path/3".into()),
            thumb_large_orig_webp: Some("/path/4".into()),
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset_id = assert_ok!(repository::asset::insert_asset(&pool, &asset).await);
    let asset2_id = assert_ok!(repository::asset::insert_asset(&pool, &asset2).await);
    let _asset3_id = assert_ok!(repository::asset::insert_asset(&pool, &asset3).await);
    let expected_ids: HashSet<AssetId> = HashSet::from([asset_id, asset2_id]);
    let result_no_limit: HashSet<AssetId> =
        assert_ok!(repository::asset::get_assets_with_missing_thumbnail(&pool, None).await)
            .into_iter()
            .map(|a| a.id)
            .collect();
    let result_limit_1: HashSet<AssetId> =
        assert_ok!(repository::asset::get_assets_with_missing_thumbnail(&pool, Some(1)).await)
            .into_iter()
            .map(|a| a.id)
            .collect();
    assert_eq!(result_no_limit, expected_ids);
    assert_eq!(result_limit_1.len(), 1);
    assert!(result_limit_1.is_subset(&expected_ids));
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
            dash_resource_dir: None,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id: root_dir2_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(3)).unwrap()),
            size: Size {
                width: 100,
                height: 100,
            },
            rotation_correction: Some(90),
            hash: None,
            thumb_small_square_avif: Some("/path/1".into()),
            thumb_small_square_webp: None,
            thumb_large_orig_avif: None,
            thumb_large_orig_webp: Some("/path/2".into()),
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset2 = Asset {
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Image,
            root_dir_id,
            file_type: "jpeg".to_owned(),
            file_path: "/path/to/image.jpg".into(),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(4)).unwrap()),
            size: Size {
                width: 1000,
                height: 1000,
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
    let asset3 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("aac".into()),
            dash_resource_dir: Some("/dash1".into()),
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
            dash_resource_dir: None,
        }),
        base: AssetBase {
            root_dir_id: root_dir2_id,
            file_path: "/some/video2.mp4".into(),
            ..asset.base.clone()
        },
    };
    let asset_id = assert_ok!(repository::asset::insert_asset(&pool, &asset).await);
    let _asset2_id = assert_ok!(repository::asset::insert_asset(&pool, &asset2).await);
    let _asset3_id = assert_ok!(repository::asset::insert_asset(&pool, &asset3).await);
    let asset4_id = assert_ok!(repository::asset::insert_asset(&pool, &asset4).await);
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
    todo!()
}

#[tokio::test]
async fn get_videos_with_no_acceptable_repr() {
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
    // video h264
    let asset = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            dash_resource_dir: None,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id: root_dir2_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(3)).unwrap()),
            size: Size {
                width: 100,
                height: 100,
            },
            rotation_correction: Some(90),
            hash: None,
            thumb_small_square_avif: Some("/path/1".into()),
            thumb_small_square_webp: None,
            thumb_large_orig_avif: None,
            thumb_large_orig_webp: Some("/path/2".into()),
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    // image
    let asset2 = Asset {
        sp: AssetSpe::Image(Image {}),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Image,
            root_dir_id,
            file_type: "jpeg".to_owned(),
            file_path: "/path/to/image.jpg".into(),
            added_at: Utc::now(),
            taken_date: MediaTimestamp::Utc(Utc::now().checked_sub_months(Months::new(4)).unwrap()),
            size: Size {
                width: 1000,
                height: 1000,
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
    // video hevc
    let asset3 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("aac".into()),
            dash_resource_dir: Some("/dash1".into()),
        }),
        base: AssetBase {
            root_dir_id: root_dir2_id,
            file_path: "/some/video.mp4".into(),
            ..asset.base.clone()
        },
    };
    // video hevc
    let asset4 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("aac".into()),
            dash_resource_dir: None,
        }),
        base: AssetBase {
            root_dir_id: root_dir2_id,
            file_path: "/some/video2.mp4".into(),
            ..asset.base.clone()
        },
    };
    // video mov mjpeg
    let asset5 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "mjpeg".to_owned(),
            video_bitrate: 123456,
            audio_codec_name: Some("pcm_u8".into()),
            dash_resource_dir: None,
        }),
        base: AssetBase {
            root_dir_id: root_dir2_id,
            file_type: "mov".to_owned(),
            file_path: "/some/video5.mov".into(),
            ..asset.base.clone()
        },
    };
    let asset_id = assert_ok!(repository::asset::insert_asset(&pool, &asset).await);
    let _asset2_id = assert_ok!(repository::asset::insert_asset(&pool, &asset2).await);
    let asset3_id = assert_ok!(repository::asset::insert_asset(&pool, &asset3).await);
    let asset4_id = assert_ok!(repository::asset::insert_asset(&pool, &asset4).await);
    let asset5_id = assert_ok!(repository::asset::insert_asset(&pool, &asset5).await);
    let acceptable_video_codecs = ["h264", "av1", "vp9", "mjpeg"];
    let acceptable_audio_codecs = ["aac", "opus", "flac", "mp3"];
    let videos_with_no_acceptable_repr: HashSet<VideoAsset> = assert_ok!(
        repository::asset::get_video_assets_with_no_acceptable_repr(
            &pool,
            acceptable_video_codecs.into_iter(),
            acceptable_audio_codecs.into_iter()
        )
        .await
    )
    .into_iter()
    .collect();
    // no reprs at all yet, so expect to get all videos
    let expected_all_videos_with_ids: HashSet<VideoAsset> = [
        Asset {
            base: AssetBase {
                id: asset_id,
                ..asset.base.clone()
            },
            ..asset.clone()
        },
        Asset {
            base: AssetBase {
                id: asset3_id,
                ..asset3.base.clone()
            },
            ..asset3.clone()
        },
        Asset {
            base: AssetBase {
                id: asset4_id,
                ..asset4.base.clone()
            },
            ..asset4.clone()
        },
        Asset {
            base: AssetBase {
                id: asset5_id,
                ..asset5.base.clone()
            },
            ..asset5.clone()
        },
    ]
    .into_iter()
    .map(|a| a.try_into().unwrap())
    .collect();
    assert_eq!(videos_with_no_acceptable_repr, expected_all_videos_with_ids);

    let asset3_repr = VideoRepresentation {
        id: VideoRepresentationId(0),
        asset_id: asset3_id,
        bitrate: 123456,
        codec_name: "av1".to_owned(),
        width: 100,
        height: 100,
        path: "/path/to/av1repr.mp4".into(),
    };
    let _asset3_repr_id = assert_ok!(
        repository::representation::insert_video_representation(
            pool.acquire().await.unwrap().as_mut(),
            &asset3_repr
        )
        .await
    );
    let asset5_repr = VideoRepresentation {
        id: VideoRepresentationId(0),
        asset_id: asset5_id,
        bitrate: 124456,
        codec_name: "av1".to_owned(),
        width: 100,
        height: 100,
        path: "/path/to/otherav1repr.mp4".into(),
    };
    let _asset5_repr_id = assert_ok!(
        repository::representation::insert_video_representation(
            pool.acquire().await.unwrap().as_mut(),
            &asset5_repr
        )
        .await
    );
    let videos_with_no_acceptable_repr: HashSet<VideoAsset> = assert_ok!(
        repository::asset::get_video_assets_with_no_acceptable_repr(
            &pool,
            acceptable_video_codecs.into_iter(),
            acceptable_audio_codecs.into_iter()
        )
        .await
    )
    .into_iter()
    .collect();
    // no audio reprs, so expect all videos
    assert_eq!(videos_with_no_acceptable_repr, expected_all_videos_with_ids);

    let asset3_audio_repr = AudioRepresentation {
        id: AudioRepresentationId(0),
        asset_id: asset3_id,
        codec_name: "aac".into(),
        path: "/audio/path/3.mp4".into(),
    };
    let _asset3_audio_repr_id = assert_ok!(
        repository::representation::insert_audio_representation(
            pool.acquire().await.unwrap().as_mut(),
            &asset3_audio_repr
        )
        .await
    );
    let asset4_audio_repr = AudioRepresentation {
        id: AudioRepresentationId(0),
        asset_id: asset4_id,
        codec_name: "pcm_u8".into(),
        path: "/audio/path/4.idk".into(),
    };
    let _asset4_audio_repr_id = assert_ok!(
        repository::representation::insert_audio_representation(
            pool.acquire().await.unwrap().as_mut(),
            &asset4_audio_repr
        )
        .await
    );
    let asset5_audio_repr = AudioRepresentation {
        id: AudioRepresentationId(0),
        asset_id: asset5_id,
        codec_name: "aac".into(),
        path: "/audio/path/5.mp4".into(),
    };
    let _asset5_audio_repr_id = assert_ok!(
        repository::representation::insert_audio_representation(
            pool.acquire().await.unwrap().as_mut(),
            &asset5_audio_repr
        )
        .await
    );
    // asset1 has no audio repr
    // asset4 only has audio in pcm_u8
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
    let videos_with_no_acceptable_repr: HashSet<VideoAsset> = assert_ok!(
        repository::asset::get_video_assets_with_no_acceptable_repr(
            &pool,
            acceptable_video_codecs.into_iter(),
            acceptable_audio_codecs.into_iter()
        )
        .await
    )
    .into_iter()
    .collect();
    assert_eq!(videos_with_no_acceptable_repr, expected);
}
