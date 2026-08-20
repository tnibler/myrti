use diesel::sql_types::BigInt;

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
        rep_file_id -> BigInt,
        is_hidden -> Integer,
        taken_date -> BigInt,
        timezone_offset -> Nullable<Text>,
        timezone_info -> Integer,
        series_id -> Nullable<BigInt>,
        is_series_selection -> Nullable<Integer>,
        gps_latitude -> Nullable<BigInt>,
        gps_longitude -> Nullable<BigInt>,
    }
}

diesel::table! {
    AssetFile (file_id) {
        file_id -> BigInt,
        asset_id -> BigInt,
        asset_type -> Integer,
        root_dir_id -> BigInt,
        file_path -> Text,
        file_type -> Text,
        hash -> Nullable<Binary>,
        added_at -> BigInt,
        width -> Integer,
        height -> Integer,
        rotation_correction -> Integer,
        mirror_correction -> Integer,
        thumb_hash -> Nullable<Blob>,
        exiftool_output -> Blob,
    }
}

diesel::table! {
    VideoFile (file_id) {
        file_id -> BigInt,
        asset_type -> Integer,
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
    ImageFile (file_id) {
        file_id -> BigInt,
        asset_type -> Integer,
        image_format_name -> Text,
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
        file_id -> BigInt,
        ty -> Integer,
        width -> Integer,
        height -> Integer,
        format_name -> Text,
    }
}

diesel::table! {
    AudioRepresentation (audio_repr_id) {
        audio_repr_id -> BigInt,
        file_id -> BigInt,
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
    DuplicateFile (dup_file_id) {
        dup_file_id -> BigInt,
        file_id -> BigInt,
        root_dir_id -> BigInt,
        file_path -> Text,
    }
}

diesel::table! {
    ImageRepresentation (image_repr_id) {
        image_repr_id -> BigInt,
        file_id -> BigInt,
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
    TimelineMonth(timeline_month_id)  {
        timeline_month_id -> BigInt,
        start_of_month -> BigInt,
        section_idx -> BigInt,
        num_assets -> Integer,
        total_width -> Double,
    }
}

diesel::table! {
    TimelineSection (section_idx) {
        section_idx -> BigInt,
        section_len -> Integer,
        total_width -> Double,
    }
}

diesel::table! {
    VideoRepresentation (video_repr_id) {
        video_repr_id -> BigInt,
        file_id -> BigInt,
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
diesel::joinable!(AssetFile -> AssetRootDir (root_dir_id));
diesel::joinable!(Asset -> AssetSeries (series_id));
diesel::joinable!(VideoFile -> AssetFile (file_id));
diesel::joinable!(ImageFile -> AssetFile (file_id));
diesel::joinable!(AssetThumbnail -> AssetFile (file_id));
diesel::joinable!(AudioRepresentation -> VideoFile (file_id));
diesel::joinable!(DuplicateFile -> AssetFile (file_id));
diesel::joinable!(DuplicateFile -> AssetRootDir (root_dir_id));
diesel::joinable!(ImageRepresentation -> ImageFile (file_id));
diesel::joinable!(TimelineGroupItem -> Asset (asset_id));
diesel::joinable!(TimelineGroupItem -> TimelineGroup (group_id));
diesel::joinable!(VideoRepresentation -> VideoFile (file_id));
diesel::joinable!(DeletedAutoAssetSeries -> Asset (asset_id));

diesel::allow_tables_to_appear_in_same_query!(
    Album,
    AlbumItem,
    AlbumThumbnail,
    Asset,
    AssetFile,
    AssetRootDir,
    AssetThumbnail,
    AudioRepresentation,
    DataDir,
    DuplicateFile,
    ImageFile,
    ImageRepresentation,
    TimelineGroup,
    TimelineGroupItem,
    AssetSeries,
    VideoRepresentation,
    TimelineSection,
    TimelineMonth,
    DeletedAutoAssetSeries,
    VideoFile,
    AcceptableVideoCodec,
    AcceptableAudioCodec,
);
