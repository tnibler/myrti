use camino::Utf8PathBuf as PathBuf;
use proptest::prelude::*;

use crate::model::{
    Album, AlbumId, AlbumItem, AlbumItemType, Asset, AssetBase, AssetId, AssetRootDirId, AssetType,
    CreateAsset, CreateAssetBase, CreateAssetImage, CreateAssetSpe, CreateAssetVideo,
    FFProbeOutput, GpsCoordinates, Image, ImageAsset, Size, TimelineGroup, TimelineGroupId,
    TimestampInfo, Video, VideoAsset,
};

fn path_strategy() -> BoxedStrategy<PathBuf> {
    r"[^\\0].{5,}".prop_map(PathBuf::from).boxed()
}

fn fixed_offset_strategy() -> BoxedStrategy<chrono::FixedOffset> {
    prop_oneof![
        (-86_399..=86_399)
            // only whole minute offset because FixedOffset::from_str() somehow throws away the
            // seconds part
            .prop_map(|secs| chrono::FixedOffset::east_opt(secs - (secs % 60)).unwrap()),
        (-86_399..=86_399)
            .prop_map(|secs| chrono::FixedOffset::west_opt(secs - (secs % 60)).unwrap()),
    ]
    .boxed()
}

pub fn arb_datetime_utc() -> BoxedStrategy<chrono::DateTime<chrono::Utc>> {
    let future = (chrono::Utc::now() + chrono::Duration::weeks(52 * 100)).timestamp();
    (0..future)
        .prop_map(|seconds| chrono::DateTime::from_timestamp(seconds, 0).unwrap())
        .boxed()
}

fn gps_coords_strategy() -> BoxedStrategy<GpsCoordinates> {
    // db stores multiplied by 10e8
    let lat = (-90_i64 * 100_000_000)..(90 * 100_000_000);
    let lon = (-180_i64 * 100_000_000)..(180 * 100_000_000);
    (lat, lon)
        .prop_map(|(lat, lon)| GpsCoordinates { lat, lon })
        .boxed()
}

pub fn timestamp_info_strategy() -> BoxedStrategy<TimestampInfo> {
    prop_oneof![
        Just(TimestampInfo::NoTimestamp),
        Just(TimestampInfo::UtcCertain),
        fixed_offset_strategy().prop_map(TimestampInfo::TzCertain),
        fixed_offset_strategy().prop_map(TimestampInfo::TzGuessedLocal),
        fixed_offset_strategy().prop_map(TimestampInfo::TzInferredLocation),
        fixed_offset_strategy().prop_map(TimestampInfo::TzSetByUser),
    ]
    .boxed()
}

prop_compose! {
    pub fn arb_new_asset_base(
        file_type: String,
    )
    (
        file_path in path_strategy().no_shrink(),
        is_hidden in any::<bool>(),
        taken_date in arb_datetime_utc(),
        timestamp_info in timestamp_info_strategy(),
        size in (200..4000_i32, 200..4000_i32).prop_map(|(w, h)| Size { width: w, height: h}),
        rotation_correction in prop_oneof![
            Just(None),
            Just(Some(90)),
            Just(Some(180)),
            Just(Some(-90)),
        ],
        gps_coordinates in prop_oneof![
            Just(None),
            gps_coords_strategy().prop_map(Some)
        ],
        hash in any::<Option<u64>>().no_shrink(),
        exiftool_output in any::<Vec<u8>>().no_shrink(),
    ) -> CreateAssetBase {
        CreateAssetBase {
            root_dir_id: AssetRootDirId(0),
            file_type: file_type.clone(),
            file_path,
            is_hidden,
            taken_date,
            timestamp_info,
            size,
            rotation_correction,
            gps_coordinates,
            hash,
            exiftool_output,
        }
    }
}

prop_compose! {
    pub fn arb_new_image_asset()
    (
        file_type in "jpeg|png|webp|avif|heic"
    )
    (
        base in arb_new_asset_base(file_type)
    ) -> CreateAsset {
        CreateAsset {
            spe: CreateAssetSpe::Image(CreateAssetImage {
                image_format_name: base.file_type.clone()
            }),
            base,
        }
    }
}

prop_compose! {
    pub fn arb_new_video_asset()
    (
        file_type in "mp4|mov|avi",
    )
    (
        base in arb_new_asset_base(file_type),
        video_codec_name in "h264|hevc|av1|vp9|mjpeg",
        video_bitrate in 800_000_i64..5_000_000,
        audio_codec_name in prop_oneof![
            1 => Just(None),
            4 => "mp3|aac|opus|pcm_u8".prop_map(Some),
        ],
        video_duration_ms in any::<Option<i64>>().no_shrink(),
        ffprobe_output in any::<Vec<u8>>().no_shrink(),
    ) -> CreateAsset {
        CreateAsset {
            base,
            spe: CreateAssetSpe::Video(CreateAssetVideo {
                video_codec_name,
                video_bitrate,
                audio_codec_name,
                has_dash: false,
                video_duration_ms,
                ffprobe_output: FFProbeOutput(ffprobe_output),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum ArbCreateAlbumItem {
    Asset(CreateAsset),
    Text(String),
}

pub fn arb_new_album_item() -> BoxedStrategy<ArbCreateAlbumItem> {
    prop_oneof![
        arb_new_asset().prop_map(ArbCreateAlbumItem::Asset),
        prop::string::string_regex("[a-zA-Z0-9 .,-]+")
            .unwrap()
            .prop_map(ArbCreateAlbumItem::Text)
    ]
    .boxed()
}

pub fn arb_new_asset() -> BoxedStrategy<CreateAsset> {
    prop_oneof![arb_new_image_asset(), arb_new_video_asset()].boxed()
}

prop_compose! {
    pub fn arb_new_timeline_group()
    (
        name in prop::option::of(".+"),
        created_at in arb_datetime_utc(),
        changed_at in arb_datetime_utc(),
        display_date in arb_datetime_utc()
    ) -> TimelineGroup {
        TimelineGroup {
            id: TimelineGroupId(0),
            name,
            created_at,
            changed_at,
            display_date,
        }
    }
}

prop_compose! {
    /// Arbitrary album with is_timeline_group=false
    pub fn arb_new_album()
    (
        name in prop::option::of(".+"),
        description in prop::option::of(".*"),
        created_at in arb_datetime_utc(),
        changed_at in arb_datetime_utc(),
    ) -> Album {
        Album {
            id: AlbumId(0),
            name,
            description,
            created_at,
            changed_at,
        }
    }
}

prop_compose! {
    pub fn arb_new_create_asset_base(
        asset_root_id: AssetRootDirId,
        file_type: String,
    )
    (
        file_path in path_strategy().no_shrink(),
        taken_date in arb_datetime_utc(),
        timestamp_info in timestamp_info_strategy(),
        size in (200..4000_i32, 200..4000_i32).prop_map(|(w, h)| Size { width: w, height: h}),
        rotation_correction in prop_oneof![
            Just(None),
            Just(Some(90)),
            Just(Some(180)),
            Just(Some(-90)),
        ],
        gps_coordinates in prop_oneof![
            Just(None),
            gps_coords_strategy().prop_map(Some)
        ],
        hash in any::<Option<u64>>().no_shrink(),
        is_hidden in any::<bool>(),
        exiftool_output in any::<Vec<u8>>().no_shrink(),
    ) -> CreateAssetBase {
        CreateAssetBase {
            root_dir_id: asset_root_id,
            file_type: file_type.clone(),
            file_path,
            taken_date,
            is_hidden,
            timestamp_info,
            size,
            rotation_correction,
            gps_coordinates,
            hash,
            exiftool_output,
        }
    }
}

// prop_compose! {
//     pub fn arb_new_create_image_asset(asset_root_dir_id: AssetRootDirId)
//     (
//         file_type in "jpeg|png|webp|avif|heic"
//     )
//     (
//         base in arb_new_create_asset_base(asset_root_dir_id, file_type)
//     ) -> CreateAsset {
//         CreateAsset {
//             spe: CreateAssetSpe::Image(CreateAssetImage  {
//                 image_format_name: base.file_type.clone()
//             }),
//             base,
//         }
//     }
// }
//
// prop_compose! {
//     pub fn arb_new_create_video_asset(asset_root_dir_id: AssetRootDirId)
//     (
//         file_type in "mp4|mov|avi",
//     )
//     (
//         base in arb_new_create_asset_base(asset_root_dir_id, file_type),
//         video_codec_name in "h264|hevc|av1|vp9|mjpeg",
//         video_bitrate in 800_000_i64..5_000_000,
//         audio_codec_name in prop_oneof![
//             1 => Just(None),
//             4 => "mp3|aac|opus|pcm_u8".prop_map(Some),
//         ],
//     ) -> CreateAsset {
//         CreateAsset {
//             base,
//             spe: CreateAssetSpe::Video(CreateAssetVideo {
//                 video_codec_name,
//                 video_bitrate,
//                 audio_codec_name,
//                 has_dash: false,
//                 ffprobe_output: Default::default(),
//             })
//         }
//     }
// }
