diesel::table! {
    Album (album_id) {
        album_id -> BigInt,
        name -> Nullable<Text>,
        description -> Nullable<Text>,
        created_at -> BigInt,
        changed_at -> BigInt,
    }
}

diesel::table! {
    AlbumItem (album_item_id) {
        album_item_id -> BigInt,
        album_id -> BigInt,
        ty -> Integer,
        asset_id -> Nullable<BigInt>,
        text -> Nullable<Text>,
        idx -> Integer,
    }
}

diesel::table! {
    Asset (asset_id) {
        asset_id -> BigInt,
        asset_type -> Integer,
        root_dir_id -> BigInt,
        file_path -> Text,
        file_type -> Text,
        hash -> Nullable<Binary>,
        is_hidden -> Integer,
        added_at -> BigInt,
        taken_date -> BigInt,
        timezone_offset -> Nullable<Text>,
        timezone_info -> Integer,
        width -> Integer,
        height -> Integer,
        rotation_correction -> Nullable<Integer>,
        thumb_hash -> Nullable<Blob>,
        series_id -> Nullable<BigInt>,
        is_series_selection -> Nullable<Integer>,
        exiftool_output -> Blob,
        gps_latitude -> Nullable<BigInt>,
        gps_longitude -> Nullable<BigInt>,

        motion_photo -> Integer,
        motion_photo_assoc_asset_id -> Nullable<BigInt>,
        motion_photo_pts_us -> Nullable<BigInt>,
        motion_photo_video_file_id -> Nullable<BigInt>,
    }
}

diesel::table! {
    VideoAsset (video_asset_id) {
        video_asset_id -> BigInt,
        asset_type -> Integer,
        asset_id -> BigInt,
        ffprobe_output -> Binary,
        video_codec_name -> Text,
        video_bitrate -> Nullable<BigInt>,
        video_duration_ms -> Nullable<BigInt>,
        audio_codec_name -> Nullable<Text>,
        has_ghi -> Nullable<Integer>,
        max_iframe_interval -> Nullable<Integer>,
        is_original_streamable -> Nullable<Integer>,
        frame_rate_num -> Nullable<Integer>,
        frame_rate_denom -> Nullable<Integer>,
    }
}

diesel::table! {
    ImageAsset (image_asset_id) {
        image_asset_id -> BigInt,
        asset_type -> Integer,
        asset_id -> BigInt,
        image_format_name -> Text,
    }
}

diesel::table! {
    MotionPhoto (motion_photo_id) {
        motion_photo_id -> BigInt,
        image_asset_id -> BigInt,
        video_asset_id -> Nullable<BigInt>,
        photo_pts_us -> Nullable<BigInt>,
    }
}

diesel::table! {
    AssetRootDir (asset_root_dir_id) {
        asset_root_dir_id -> BigInt,
        path -> Text,
    }
}

diesel::table! {
    AssetThumbnail (thumbnail_id) {
        thumbnail_id -> BigInt,
        asset_id -> BigInt,
        ty -> Integer,
        width -> Integer,
        height -> Integer,
        format_name -> Text,
    }
}

diesel::table! {
    AudioRepresentation (audio_repr_id) {
        audio_repr_id -> BigInt,
        video_asset_id -> BigInt,
        name -> Text,
        created_status -> Integer,
        codec_name -> Text,
    }
}

diesel::table! {
    DataDir (id) {
        id -> Integer,
        path -> Text,
    }
}

diesel::table! {
    DuplicateAsset (dup_asset_id) {
        dup_asset_id -> BigInt,
        asset_id -> BigInt,
        root_dir_id -> BigInt,
        file_path -> Text,
    }
}

diesel::table! {
    FailedFFmpeg (asset_id) {
        asset_id -> BigInt,
        file_hash -> Binary,
        date -> BigInt,
    }
}

diesel::table! {
    FailedShakaPackager (asset_id) {
        asset_id -> BigInt,
        file_hash -> Binary,
        date -> BigInt,
    }
}

diesel::table! {
    FailedThumbnailJob (asset_id) {
        asset_id -> BigInt,
        file_hash -> Binary,
        date -> BigInt,
    }
}

diesel::table! {
    ImageRepresentation (image_repr_id) {
        image_repr_id -> BigInt,
        image_asset_id -> BigInt,
        format_name -> Text,
        width -> Integer,
        height -> Integer,
        file_size -> BigInt,
        file_key -> Text,
    }
}

diesel::table! {
    AlbumThumbnail (thumbnail_id) {
        thumbnail_id -> BigInt,
        album_id -> BigInt,
        format_name -> Text,
        width -> Integer,
        height -> Integer,
        file_key -> Text,
    }
}

diesel::table! {
    AssetSeries (series_id) {
        series_id -> BigInt,
        is_auto -> Integer,
    }
}

diesel::table! {
    DeletedAutoAssetSeries (id) {
        id -> BigInt,
        asset_id -> BigInt,
        series_id -> BigInt,
    }
}

diesel::table! {
    TimelineGroup (timeline_group_id) {
        timeline_group_id -> BigInt,
        name -> Nullable<Text>,
        display_date -> BigInt,
        created_at -> BigInt,
        changed_at -> BigInt,
    }
}

diesel::table! {
    TimelineGroupItem (timeline_group_item_id) {
        timeline_group_item_id -> BigInt,
        group_id -> BigInt,
        asset_id -> BigInt,
    }
}

diesel::table! {
    VideoRepresentation (video_repr_id) {
        video_repr_id -> BigInt,
        video_asset_id -> BigInt,
        name -> Text,
        created_status -> Integer,
        codec_name -> Text,
        width -> Nullable<Integer>,
        height -> Nullable<Integer>,
        bitrate -> Nullable<BigInt>,
    }
}

diesel::table! {
    AcceptableVideoCodec (codec_name) {
        codec_name -> Text,
    }
}

diesel::table! {
    AcceptableAudioCodec (codec_name) {
        codec_name -> Text,
    }
}

diesel::joinable!(AlbumItem -> Album (album_id));
diesel::joinable!(AlbumItem -> Asset (asset_id));
diesel::joinable!(AlbumThumbnail -> Album (album_id));
diesel::joinable!(Asset -> AssetRootDir (root_dir_id));
diesel::joinable!(Asset -> AssetSeries (series_id));
diesel::joinable!(VideoAsset -> Asset (asset_id));
diesel::joinable!(ImageAsset -> Asset (asset_id));
diesel::joinable!(AssetThumbnail -> Asset (asset_id));
diesel::joinable!(AudioRepresentation -> VideoAsset (video_asset_id));
diesel::joinable!(DuplicateAsset -> Asset (asset_id));
diesel::joinable!(DuplicateAsset -> AssetRootDir (root_dir_id));
diesel::joinable!(ImageRepresentation -> ImageAsset (image_asset_id));
diesel::joinable!(TimelineGroupItem -> Asset (asset_id));
diesel::joinable!(TimelineGroupItem -> TimelineGroup (group_id));
diesel::joinable!(VideoRepresentation -> VideoAsset (video_asset_id));
diesel::joinable!(DeletedAutoAssetSeries -> Asset (asset_id));

diesel::allow_tables_to_appear_in_same_query!(
    Album,
    AlbumItem,
    AlbumThumbnail,
    Asset,
    AssetRootDir,
    AssetThumbnail,
    AudioRepresentation,
    DataDir,
    DuplicateAsset,
    FailedFFmpeg,
    FailedShakaPackager,
    FailedThumbnailJob,
    ImageAsset,
    ImageRepresentation,
    TimelineGroup,
    TimelineGroupItem,
    AssetSeries,
    VideoRepresentation,
    DeletedAutoAssetSeries,
    MotionPhoto,
    VideoAsset,
);
