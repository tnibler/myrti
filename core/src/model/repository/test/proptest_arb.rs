use camino::Utf8PathBuf as PathBuf;
use proptest::prelude::*;

use crate::model::{
    Album, AlbumId, AlbumType, Asset, AssetBase, AssetId, AssetRootDirId, AssetSpe, AssetType,
    GpsCoordinates, Image, ImageAsset, Size, TimelineGroup, TimelineGroupAlbum, TimestampInfo,
    Video, VideoAsset,
};

fn path_strategy() -> BoxedStrategy<PathBuf> {
    r"[^\\0].{5,}".prop_map(|s| PathBuf::from(s)).boxed()
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
        fixed_offset_strategy().prop_map(|offset| TimestampInfo::TzCertain(offset)),
        fixed_offset_strategy().prop_map(|offset| TimestampInfo::TzGuessedLocal(offset)),
        fixed_offset_strategy().prop_map(|offset| TimestampInfo::TzInferredLocation(offset)),
        fixed_offset_strategy().prop_map(|offset| TimestampInfo::TzSetByUser(offset)),
    ]
    .boxed()
}

prop_compose! {
    pub fn arb_new_asset_base(
        asset_root_id: AssetRootDirId,
        ty: AssetType,
        file_type: String,
    )
    (
        file_path in path_strategy().no_shrink(),
        is_hidden in any::<bool>(),
        added_at in arb_datetime_utc(),
        taken_date in arb_datetime_utc(),
        timestamp_info in timestamp_info_strategy(),
        size in (200..4000_i64, 200..4000_i64).prop_map(|(w, h)| Size { width: w, height: h}),
        rotation_correction in prop_oneof![
            Just(None),
            Just(Some(90)),
            Just(Some(180)),
            Just(Some(-90)),
        ],
        gps_coordinates in prop_oneof![
            Just(None),
            gps_coords_strategy().prop_map(|coords| Some(coords))
        ],
        hash in any::<Option<u64>>().no_shrink(),
    ) -> AssetBase {
        AssetBase {
            id: AssetId(0),
            ty,
            root_dir_id: asset_root_id,
            file_type: file_type.clone(),
            file_path,
            is_hidden,
            added_at,
            taken_date,
            timestamp_info,
            size,
            rotation_correction,
            gps_coordinates,
            hash,
            thumb_small_square_avif: false,
            thumb_small_square_webp: false,
            thumb_large_orig_avif: false,
            thumb_large_orig_webp: false,
            thumb_large_orig_size: None,
            thumb_small_square_size: None
        }
    }
}

prop_compose! {
    pub fn arb_new_image_asset(asset_root_dir_id: AssetRootDirId)
    (
        file_type in "jpeg|png|webp|avif|heic"
    )
    (
        base in arb_new_asset_base(asset_root_dir_id, AssetType::Image, file_type)
    ) -> ImageAsset {
        ImageAsset {
            image: Image {
                image_format_name: base.file_type.clone()
            },
            base,
        }
    }
}

prop_compose! {
    pub fn arb_new_video_asset(asset_root_dir_id: AssetRootDirId)
    (
        file_type in "mp4|mov|avi",
    )
    (
        base in arb_new_asset_base(asset_root_dir_id, AssetType::Video, file_type),
        video_codec_name in "h264|hevc|av1|vp9|mjpeg",
        video_bitrate in 800_000_i64..5_000_000,
        audio_codec_name in prop_oneof![
            1 => Just(None),
            4 => "mp3|aac|opus|pcm_u8".prop_map(|codec| Some(codec)),
        ],
    ) -> VideoAsset {
        VideoAsset {
            base,
            video: Video {
                video_codec_name,
                video_bitrate,
                audio_codec_name,
                has_dash: false
            }
        }
    }
}

pub fn arb_new_asset(asset_root_dir_id: AssetRootDirId) -> BoxedStrategy<Asset> {
    prop_oneof![
        arb_new_image_asset(asset_root_dir_id).prop_map(|image| image.into()),
        arb_new_video_asset(asset_root_dir_id).prop_map(|video| video.into())
    ]
    .boxed()
}

prop_compose! {
    /// Arbitrary album with is_timeline_group=false
    pub fn arb_new_album()
    (
        name in prop::option::of(".+"),
        description in prop::option::of(".*"),
        created_at in arb_datetime_utc(),
        changed_at in arb_datetime_utc(),
        timeline_group_display_date in prop::option::of(arb_datetime_utc())
    ) -> AlbumType {
        let base = Album {
            id: AlbumId(0),
            name,
            description,
            created_at,
            changed_at,
        };
        match timeline_group_display_date {
            None => AlbumType::Album(base),
            Some(tgdd) => AlbumType::TimelineGroup(
                TimelineGroupAlbum {
                    album: base,
                    group: TimelineGroup { display_date: tgdd }
                }
            )

        }
    }
}
