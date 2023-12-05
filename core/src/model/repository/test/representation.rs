use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use chrono::Months;
use claims::assert_ok;
use pretty_assertions::assert_eq;

use crate::{catalog::storage_key, model::*};

pub use super::*;

#[tokio::test]
async fn insert_retrieve_video_representation() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: false,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            is_hidden: false,
            added_at: utc_now_millis_zero(),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1023,
            },
            rotation_correction: None,
            gps_coordinates: None,
            hash: None,
            thumb_small_square_avif: false,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: false,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset2 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 456,
            audio_codec_name: Some("opus".into()),
            has_dash: false,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video2.mp4"),
            is_hidden: false,
            added_at: utc_now_millis_zero(),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1023,
            },
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
            thumb_small_square_avif: false,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: false,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset_id = assert_ok!(repository::asset::insert_asset(&pool, &asset).await);
    let asset2_id = assert_ok!(repository::asset::insert_asset(&pool, &asset2).await);
    let video_repr = VideoRepresentation {
        id: VideoRepresentationId(0),
        asset_id,
        codec_name: "av1".to_owned(),
        bitrate: 123456,
        width: 123,
        height: 456,
        file_key: storage_key::dash_file(asset_id, format_args!("av1_100x100.mp4")),
        media_info_key: storage_key::dash_file(
            asset_id,
            format_args!("av1_100x100.mp4.media_info"),
        ),
    };
    let video_repr2 = VideoRepresentation {
        id: VideoRepresentationId(0),
        asset_id,
        codec_name: "av1".to_owned(),
        bitrate: 123456,
        width: 1230,
        height: 4560,
        file_key: storage_key::dash_file(asset_id, format_args!("av1_1230x4560.mp4")),
        media_info_key: storage_key::dash_file(
            asset_id,
            format_args!("av1_1230x4560.mp4.media_info"),
        ),
    };
    let video_repr3 = VideoRepresentation {
        id: VideoRepresentationId(0),
        asset_id: asset2_id,
        codec_name: "av1".to_owned(),
        bitrate: 12345,
        width: 230,
        height: 560,
        file_key: storage_key::dash_file(asset2_id, format_args!("av1_1230x4560.mp4")),
        media_info_key: storage_key::dash_file(
            asset2_id,
            format_args!("av1_1230x4560.mp4.media_info"),
        ),
    };
    let video_repr_id = assert_ok!(
        repository::representation::insert_video_representation(
            pool.acquire().await.unwrap().as_mut(),
            &video_repr
        )
        .await
    );
    let video_repr2_id = assert_ok!(
        repository::representation::insert_video_representation(
            pool.acquire().await.unwrap().as_mut(),
            &video_repr2
        )
        .await
    );
    let _video_repr3_id = assert_ok!(
        repository::representation::insert_video_representation(
            pool.acquire().await.unwrap().as_mut(),
            &video_repr3
        )
        .await
    );
    let video_repr_with_id = VideoRepresentation {
        id: video_repr_id,
        ..video_repr
    };
    let video_repr2_with_id = VideoRepresentation {
        id: video_repr2_id,
        ..video_repr2
    };
    let retrieved: HashSet<_> =
        assert_ok!(repository::representation::get_video_representations(&pool, asset_id).await)
            .into_iter()
            .collect();
    let expected: HashSet<_> = [video_repr_with_id, video_repr2_with_id]
        .into_iter()
        .collect();
    assert_eq!(retrieved, expected);
}

#[tokio::test]
async fn insert_retrieve_audio_representation() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "h264".to_owned(),
            video_bitrate: 1234,
            audio_codec_name: Some("aac".into()),
            has_dash: false,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
            is_hidden: false,
            added_at: utc_now_millis_zero(),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1023,
            },
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
            thumb_small_square_avif: false,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: false,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset2 = Asset {
        sp: AssetSpe::Video(Video {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: 456,
            audio_codec_name: Some("mp3".into()),
            has_dash: false,
        }),
        base: AssetBase {
            id: AssetId(0),
            ty: AssetType::Video,
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video2.mp4"),
            is_hidden: false,
            added_at: utc_now_millis_zero(),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1023,
            },
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
            thumb_small_square_avif: false,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: false,
            thumb_small_square_size: None,
            thumb_large_orig_size: None,
        },
    };
    let asset_id = assert_ok!(repository::asset::insert_asset(&pool, &asset).await);
    let asset2_id = assert_ok!(repository::asset::insert_asset(&pool, &asset2).await);
    let audio_repr = AudioRepresentation {
        id: AudioRepresentationId(0),
        asset_id,
        codec_name: "opus".into(),
        file_key: storage_key::dash_file(asset_id, format_args!("audio.mp4")),
        media_info_key: storage_key::dash_file(asset_id, format_args!("audio.mp4.media_info")),
    };
    let audio_repr2 = AudioRepresentation {
        id: AudioRepresentationId(0),
        asset_id: asset2_id,
        codec_name: "flac".into(),
        file_key: storage_key::dash_file(asset2_id, format_args!("audio.mp4")),
        media_info_key: storage_key::dash_file(asset2_id, format_args!("audio.mp4.media_info")),
    };
    let audio_repr_id = assert_ok!(
        repository::representation::insert_audio_representation(
            pool.acquire().await.unwrap().as_mut(),
            &audio_repr
        )
        .await
    );
    let _audio_repr2_id = assert_ok!(
        repository::representation::insert_audio_representation(
            pool.acquire().await.unwrap().as_mut(),
            &audio_repr2
        )
        .await
    );
    let audio_repr_with_id = AudioRepresentation {
        id: audio_repr_id,
        ..audio_repr
    };
    let retrieved: HashSet<_> =
        assert_ok!(repository::representation::get_audio_representation(&pool, asset_id).await)
            .into_iter()
            .collect();
    let expected: HashSet<_> = [audio_repr_with_id].into_iter().collect();
    assert_eq!(retrieved, expected);
}
