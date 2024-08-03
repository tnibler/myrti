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
        ty -> Integer,
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
        exiftool_output -> Blob,
        gps_latitude -> Nullable<BigInt>,
        gps_longitude -> Nullable<BigInt>,
        image_format_name -> Nullable<Text>,
        ffprobe_output -> Nullable<Binary>,
        video_codec_name -> Nullable<Text>,
        video_bitrate -> Nullable<BigInt>,
        audio_codec_name -> Nullable<Text>,
        has_dash -> Nullable<Integer>,
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
        asset_id -> BigInt,
        codec_name -> Text,
        file_key -> Text,
        media_info_key -> Text,
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
        asset_id -> BigInt,
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
        asset_id -> BigInt,
        codec_name -> Text,
        width -> Integer,
        height -> Integer,
        bitrate -> BigInt,
        file_key -> Text,
        media_info_key -> Text,
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
diesel::joinable!(AssetThumbnail -> Asset (asset_id));
diesel::joinable!(AudioRepresentation -> Asset (asset_id));
diesel::joinable!(DuplicateAsset -> Asset (asset_id));
diesel::joinable!(DuplicateAsset -> AssetRootDir (root_dir_id));
diesel::joinable!(ImageRepresentation -> Asset (asset_id));
diesel::joinable!(TimelineGroupItem -> Asset (asset_id));
diesel::joinable!(TimelineGroupItem -> TimelineGroup (group_id));
diesel::joinable!(VideoRepresentation -> Asset (asset_id));

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
    ImageRepresentation,
    TimelineGroup,
    TimelineGroupItem,
    VideoRepresentation,
);
