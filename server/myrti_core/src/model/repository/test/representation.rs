use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use chrono::Months;
use claims::assert_ok;
use pretty_assertions::assert_eq;

use crate::{catalog::storage_key, model::*};

pub use super::*;

#[test]
fn insert_retrieve_video_representation() {
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
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "h264".to_owned(),
            video_bitrate: Some(1234),
            audio_codec_name: Some("aac".into()),
            ffprobe_output: Default::default(),
            video_duration_ms: None,
            is_original_streamable: true,
            max_iframe_interval: Some(100),
            frame_rate: Some((300, 1000)),
        }),
        base: CreateAssetBase {
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
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
            gps_coordinates: None,
            hash: None,
            exiftool_output: Default::default(),
        },
    };
    let asset2 = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: None,
            audio_codec_name: Some("opus".into()),
            ffprobe_output: Default::default(),
            video_duration_ms: Some(14444),
            is_original_streamable: false,
            max_iframe_interval: None,
            frame_rate: Some((100, 33)),
        }),
        base: CreateAssetBase {
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video2.mp4"),
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
            hash: None,
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&mut conn, asset));
    let asset = assert_ok!(repository::asset::get_asset(&mut conn, asset_id));
    let asset_video = match asset.sp.clone() {
        AssetSpe::Image(_) => panic!(),
        AssetSpe::Video(video) => video,
    };

    let asset2_id = assert_ok!(repository::asset::create_asset(&mut conn, asset2));
    let asset2 = assert_ok!(repository::asset::get_asset(&mut conn, asset2_id));
    let asset2_video = match asset2.sp.clone() {
        AssetSpe::Image(_) => panic!(),
        AssetSpe::Video(video) => video,
    };
    let video_repr = VideoRepresentation {
        id: VideoRepresentationId(0),
        video_file_id: asset_video.video_file_id,
        codec_name: "av1".to_owned(),
        bitrate: 123456,
        width: 123,
        height: 456,
        name: format!("av1_100x100"),
    };
    let video_repr2 = VideoRepresentation {
        id: VideoRepresentationId(0),
        video_file_id: asset_video.video_file_id,
        codec_name: "av1".to_owned(),
        bitrate: 123456,
        width: 1230,
        height: 4560,
        name: format!("av1_1230x4560"),
    };
    let video_repr3 = VideoRepresentation {
        id: VideoRepresentationId(0),
        video_file_id: asset2_video.video_file_id,
        codec_name: "av1".to_owned(),
        bitrate: 12345,
        width: 230,
        height: 560,
        name: format!("av1_1230x4560"),
    };
    let video_repr_id = assert_ok!(repository::representation::insert_video_representation(
        &mut conn,
        &CreateVideoRepresentation {
            video_file_id: asset_video.video_file_id,
            name: video_repr.name.clone(),
            codec_name: video_repr.codec_name.clone()
        }
    ));
    let video_repr2_id = assert_ok!(repository::representation::insert_video_representation(
        &mut conn,
        &CreateVideoRepresentation {
            video_file_id: asset_video.video_file_id,
            name: video_repr2.name.clone(),
            codec_name: video_repr2.codec_name.clone()
        }
    ));
    let _video_repr3_id = assert_ok!(repository::representation::insert_video_representation(
        &mut conn,
        &CreateVideoRepresentation {
            video_file_id: asset2_video.video_file_id,
            name: video_repr3.name.clone(),
            codec_name: video_repr3.codec_name.clone()
        }
    ));
    let video_repr_with_id = VideoRepresentation {
        id: video_repr_id,
        ..video_repr
    };
    assert_ok!(repository::representation::finalize_video_representation(
        &mut conn,
        &video_repr_with_id
    ));
    let video_repr2_with_id = VideoRepresentation {
        id: video_repr2_id,
        ..video_repr2
    };
    assert_ok!(repository::representation::finalize_video_representation(
        &mut conn,
        &video_repr2_with_id
    ));
    let retrieved: HashSet<_> = assert_ok!(repository::representation::get_video_representations(
        &mut conn,
        asset_video.video_file_id
    ))
    .into_iter()
    .collect();
    let expected: HashSet<_> = [video_repr_with_id, video_repr2_with_id]
        .into_iter()
        .collect();
    assert_eq!(retrieved, expected);
}

#[test]
fn insert_retrieve_audio_representation() {
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
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "h264".to_owned(),
            video_bitrate: Some(1234),
            audio_codec_name: Some("aac".into()),
            video_duration_ms: None,
            ffprobe_output: Default::default(),
            is_original_streamable: true,
            max_iframe_interval: Some(33),
            frame_rate: Some((2343, 444)),
        }),
        base: CreateAssetBase {
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video.mp4"),
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
            hash: None,
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset2 = CreateAsset {
        spe: CreateAssetSpe::Video(CreateAssetVideo {
            video_codec_name: "hevc".to_owned(),
            video_bitrate: Some(456),
            audio_codec_name: Some("mp3".into()),
            ffprobe_output: Default::default(),
            video_duration_ms: Some(123433423),
            is_original_streamable: false,
            max_iframe_interval: Some(33),
            frame_rate: Some((2343, 444)),
        }),
        base: CreateAssetBase {
            root_dir_id,
            file_type: "mp4".to_owned(),
            file_path: PathBuf::from("video2.mp4"),
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
            hash: None,
            gps_coordinates: None,
            exiftool_output: Default::default(),
        },
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&mut conn, asset));
    let asset = assert_ok!(repository::asset::get_asset(&mut conn, asset_id));
    let asset_video = match asset.sp.clone() {
        AssetSpe::Image(_) => panic!(),
        AssetSpe::Video(video) => video,
    };

    let asset2_id = assert_ok!(repository::asset::create_asset(&mut conn, asset2));
    let asset2 = assert_ok!(repository::asset::get_asset(&mut conn, asset2_id));
    let asset2_video = match asset2.sp.clone() {
        AssetSpe::Image(_) => panic!(),
        AssetSpe::Video(video) => video,
    };
    let audio_repr = AudioRepresentation {
        id: AudioRepresentationId(0),
        video_file_id: asset_video.video_file_id,
        name: "opus".into(),
        codec_name: "opus".into(),
    };
    let audio_repr2 = AudioRepresentation {
        id: AudioRepresentationId(0),
        video_file_id: asset2_video.video_file_id,
        codec_name: "flac".into(),
        name: "flac".into(),
    };
    let audio_repr_id = assert_ok!(repository::representation::insert_audio_representation(
        &mut conn,
        &CreateAudioRepresentation {
            video_file_id: asset_video.video_file_id,
            name: audio_repr.name.clone(),
            codec_name: audio_repr.codec_name.clone()
        }
    ));
    let audio_repr2_id = assert_ok!(repository::representation::insert_audio_representation(
        &mut conn,
        &CreateAudioRepresentation {
            video_file_id: asset2_video.video_file_id,
            name: audio_repr2.name.clone(),
            codec_name: audio_repr2.codec_name.clone()
        }
    ));
    assert_ok!(repository::representation::finalize_audio_representation(
        &mut conn,
        audio_repr_id
    ));
    assert_ok!(repository::representation::finalize_audio_representation(
        &mut conn,
        audio_repr2_id
    ));
    let audio_repr_with_id = AudioRepresentation {
        id: audio_repr_id,
        ..audio_repr
    };
    let retrieved: HashSet<_> = assert_ok!(repository::representation::get_audio_representations(
        &mut conn,
        asset_video.video_file_id
    ))
    .into_iter()
    .collect();
    let expected: HashSet<_> = [audio_repr_with_id].into_iter().collect();
    assert_eq!(retrieved, expected);
}
