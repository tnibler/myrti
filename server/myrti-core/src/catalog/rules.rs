use std::collections::HashSet;

use eyre::{Context, Result};
use itertools::Itertools;

use myrti_data::db::PooledDbConn;
use myrti_data::{interact, repository};

use myrti_data::model::{
    AlbumId, AssetFile, AssetThumbnail, FileId, IsOriginalStreamable, OriginalStreaming,
    ThumbnailFormat, ThumbnailType, Video,
};
use myrti_data::repository::asset::AssetHasThumbnails;

use crate::{
    catalog::{
        encoding_target::{CodecTarget, VideoEncodingTarget, audio_codec_name, av1},
        operation::package_video::{AudioEncodingTarget, PackageVideoTask},
        storage_key,
    },
    config,
};

use super::{
    image_conversion_target::{ImageConversionTarget, heif::AvifTarget},
    operation::{
        convert_image::ConvertImage, create_album_thumbnail::CreateAlbumThumbnail,
        create_thumbnail::CreateAssetThumbnail, package_video::PackageVideo,
    },
};

pub async fn required_video_packaging_for_asset(
    conn: &mut PooledDbConn,
    file_id: FileId,
    bin_paths: &config::BinPaths,
) -> Result<Vec<PackageVideo>> {
    let acceptable_video_codecs = ["h264", "av1", "vp9"];
    let acceptable_audio_codecs = ["aac", "opus", "flac", "mp3"];
    // TODO yes we're clearing and putting the config back in every time lol
    interact!(conn, move |conn| {
        repository::config::set_acceptable_audio_codecs(conn, acceptable_audio_codecs)?;
        repository::config::set_acceptable_video_codecs(conn, acceptable_video_codecs)?;
        Ok(())
    })
    .await??;

    let (video, file) = interact!(conn, move |conn| {
        repository::asset::get_video_file(conn, file_id)
    })
    .await??;
    let existing_video_reprs = interact!(conn, move |conn| {
        repository::representation::get_video_representations(conn, file_id)
    })
    .await??;
    let has_acceptable_video_repr = existing_video_reprs
        .iter()
        .any(|repr| acceptable_video_codecs.contains(&repr.codec_name.as_str()));
    let audio_reprs = interact!(conn, move |conn| {
        repository::representation::get_audio_representations(conn, file_id)
    })
    .await??;
    let has_acceptable_audio_repr = video.audio_codec_name.is_none()
        || audio_reprs
            .iter()
            .any(|repr| acceptable_audio_codecs.contains(&repr.codec_name.as_str()));
    if has_acceptable_video_repr && has_acceptable_audio_repr {
        return Ok(Default::default());
    }

    let mut ops = Vec::new();

    let (original_streamable, original_streaming_task) =
        check_create_ghi_task(conn, &file, &video).await?;
    ops.extend(original_streaming_task);

    if !original_streamable.includes_video() && !has_acceptable_video_repr {
        let repr_name = format!("{}x{}", file.size.width, file.size.height);
        let video_out_key = storage_key::dash_file(file_id, format_args!("{}", repr_name));
        let keyframe_interval = match video.frame_rate {
            Some((num, denom)) if num > 0 && denom > 0 => {
                let fr = (num as f32) / (denom as f32);
                (fr.round() as i32).clamp(2, 240)
            }
            Some(_) => {
                tracing::warn!(?file_id, ?video.frame_rate, "garbage frame rate");
                30
            }
            None => 30,
        };
        ops.push(PackageVideo {
            file_id,
            repr_name,
            task: PackageVideoTask::TranscodeVideo(VideoEncodingTarget {
                codec: CodecTarget::AV1(av1::AV1Target {
                    crf: av1::Crf::default(),
                    fast_decode: None,
                    preset: None,
                    max_bitrate: None,
                }),
                scale: None,
                force_keyframe_interval: Some(keyframe_interval),
            }),
            output_key: video_out_key,
        });
    };
    if !original_streamable.includes_audio() && !has_acceptable_audio_repr {
        let repr_name = audio_codec_name(&AudioEncodingTarget::AAC);
        ops.push(PackageVideo {
            file_id,
            repr_name,
            task: PackageVideoTask::TranscodeAudio(AudioEncodingTarget::AAC),
            output_key: storage_key::dash_file(file_id, format_args!("audio_aac.mp4")),
        });
    };
    Ok(ops)
}

pub async fn required_image_conversion_for_asset(
    conn: &mut PooledDbConn,
    file_id: FileId,
) -> Result<Vec<ConvertImage>> {
    let (image, _file) = interact!(conn, move |conn| {
        repository::asset::get_image_file(conn, file_id)
    })
    .await??;
    let acceptable_formats = ["jpeg", "avif", "png", "webp"];
    if acceptable_formats.contains(&image.image_format_name.as_str()) {
        return Ok(Default::default());
    }
    let existing_image_reprs = interact!(conn, move |conn| {
        repository::representation::get_image_representations(conn, file_id)
    })
    .await??;

    if !existing_image_reprs
        .into_iter()
        .any(|repr| acceptable_formats.contains(&repr.format_name.as_str()))
    {
        let target = ImageConversionTarget {
            scale: None,
            format: super::image_conversion_target::ImageFormatTarget::AVIF(AvifTarget::default()),
        };
        let output_file_key = storage_key::image_representation(file_id, &target);
        Ok(vec![ConvertImage {
            file_id: image.file_id,
            target,
            output_file_key,
        }])
    } else {
        Ok(Default::default())
    }
}

pub async fn files_missing_thumbhash(conn: &mut PooledDbConn) -> Result<Vec<FileId>> {
    interact!(conn, move |conn| {
        repository::asset::get_files_without_thumbhash(conn)
    })
    .await?
}

pub async fn required_thumbnails_for_asset(
    conn: &mut PooledDbConn,
    file_id: FileId,
) -> Result<CreateAssetThumbnail> {
    let have_thumbnails = interact!(conn, move |conn| {
        repository::asset::get_thumbnails_for_asset(conn, file_id)
    })
    .await??;
    let (create_formats, create_types) = missing_asset_thumbnails(&have_thumbnails);
    Ok(CreateAssetThumbnail {
        file_id,
        formats: create_formats,
        thumbnail_types: create_types,
    })
}

pub async fn thumbnails_to_create(conn: &mut PooledDbConn) -> Result<Vec<CreateAssetThumbnail>> {
    // always create all thumbnails if any are missing for now
    let limit: Option<i64> = None;
    let assets_missing_thumbnails: Vec<AssetHasThumbnails> = interact!(conn, move |conn| {
        repository::asset::get_assets_with_missing_thumbnail(conn, limit)
            .wrap_err("could not query for Assets with missing thumbnails")
    })
    .await??;
    Ok(assets_missing_thumbnails
        .into_iter()
        .map(
            |AssetHasThumbnails {
                 file_id,
                 thumbnails,
             }| {
                let (create_formats, create_types) = missing_asset_thumbnails(&thumbnails);
                CreateAssetThumbnail {
                    file_id,
                    formats: create_formats,
                    thumbnail_types: create_types,
                }
            },
        )
        .collect())
}

pub fn missing_asset_thumbnails(
    have_thumbnails: &[AssetThumbnail],
) -> (Vec<ThumbnailFormat>, Vec<ThumbnailType>) {
    let have_sm_sq_formats: HashSet<ThumbnailFormat> = have_thumbnails
        .iter()
        .filter(|t| t.ty == ThumbnailType::SmallSquare)
        .map(|t| t.format)
        .collect();
    let have_lg_orig_formats: HashSet<ThumbnailFormat> = have_thumbnails
        .iter()
        .filter(|t| t.ty == ThumbnailType::LargeOrigAspect)
        .map(|t| t.format)
        .collect();
    let want_formats: HashSet<ThumbnailFormat> = [ThumbnailFormat::Webp, ThumbnailFormat::Avif]
        .into_iter()
        .collect();
    let mut create_formats = HashSet::new();
    let mut create_types = Vec::new();
    if have_lg_orig_formats != want_formats {
        create_formats.extend(&want_formats);
        create_types.push(ThumbnailType::LargeOrigAspect);
    }
    if have_sm_sq_formats != want_formats {
        create_formats.extend(&want_formats);
        create_types.push(ThumbnailType::SmallSquare);
    }
    (create_formats.into_iter().collect(), create_types)
}

pub async fn album_thumbnails_to_create(
    conn: &mut PooledDbConn,
) -> Result<Vec<CreateAlbumThumbnail>> {
    let albums_assets: Vec<(AlbumId, FileId)> = interact!(conn, move |conn| {
        let album_ids = repository::album_thumbnail::get_albums_with_missing_thumbnails(conn)?;
        album_ids
            .into_iter()
            .map(|album_id| {
                let first_asset_id =
                    repository::album::get_assets_in_album(conn, album_id, Some(1))?;
                Ok(first_asset_id
                    .first()
                    .map(|asset| (album_id, asset.base.rep_file_id)))
            })
            .filter_map_ok(|r| r)
            .collect::<Result<Vec<_>>>()
    })
    .await??;
    Ok(albums_assets
        .into_iter()
        .map(|(album_id, file_id)| CreateAlbumThumbnail {
            album_id,
            file_id,
            size: 400,
        })
        .collect())
}

pub async fn video_packaging_due(conn: &mut PooledDbConn) -> Result<Vec<PackageVideo>> {
    // priority:
    //  - videos with original in acceptable codec and no DASH packaged
    //  - videos with no representation in acceptable codec
    //  - videos with any representation from their quality ladder missing
    //    (hightest qualities come first)
    // For now, scan through all videos to check.
    // Later, set a flag if all required transcoding has been done
    // and clear the flag when the config (quality ladder, acceptable codecs)
    // change and recheck
    //
    // If we have a lot of video at the same time (e.g. initial index), we might not want to do this
    // if disk space is limited and prefer transcoding to a more efficient codec first.
    // Also only package originals if there is space for the original codec + transcode,
    // and allow setting this per storage provider (don't want to upload loads to S3 only to
    // delete it later when transcoding is done)

    let acceptable_video_codecs = ["h264", "av1", "vp9"];
    let acceptable_audio_codecs = ["aac", "opus", "flac", "mp3"];
    // TODO yes we're clearing and putting the config back in every time lol
    interact!(conn, move |conn| {
        repository::config::set_acceptable_audio_codecs(conn, acceptable_audio_codecs)?;
        repository::config::set_acceptable_video_codecs(conn, acceptable_video_codecs)?;
        Ok(())
    })
    .await??;

    let no_ghi_index = interact!(conn, move |conn| {
        repository::asset::get_video_files_with_original_streaming_due(conn)
    })
    .await??;
    let ghi_tasks = {
        let mut tasks = Vec::default();
        for (file, video) in no_ghi_index {
            match check_create_ghi_task(conn, &file, &video).await? {
                (_, Some(create_ghi)) => tasks.push(create_ghi),
                (IsOriginalStreamable::None, None) => {}
                (has_ghi_index, None) => {
                    tracing::error!(
                        ?video,
                        ?has_ghi_index,
                        "file was included in video_files_with_unknown_original_streaming query but no corresponding task is due"
                    );
                }
            }
        }
        tasks
    };

    let no_good_audio = interact!(conn, move |conn| {
        repository::asset::get_video_files_with_no_acceptable_audio_repr(conn)
    })
    .await??;
    let audio_tasks = no_good_audio.into_iter().map(|(asset_file, _video)| {
        let repr_name = String::from("audio");
        let audio_out_key = storage_key::dash_file(asset_file.id, format_args!("audio.mp4"));
        let audio_target = AudioEncodingTarget::AAC;
        PackageVideo {
            file_id: asset_file.id,
            task: PackageVideoTask::TranscodeAudio(audio_target),
            repr_name,
            output_key: audio_out_key,
        }
    });
    let no_good_video = interact!(conn, move |conn| {
        repository::asset::get_video_files_with_no_acceptable_video_repr(conn)
    })
    .await??;
    let video_tasks = no_good_video.into_iter().map(|(asset_file, _video)| {
        let repr_name = format!("{}x{}", asset_file.size.width, asset_file.size.height);
        let video_out_key = storage_key::dash_file(
            asset_file.id,
            format_args!("{}x{}.mp4", asset_file.size.width, asset_file.size.height),
        );
        let video_target = VideoEncodingTarget {
            codec: CodecTarget::AV1(av1::AV1Target {
                crf: av1::Crf::default(),
                fast_decode: None,
                preset: None,
                max_bitrate: None,
            }),
            scale: None,
            force_keyframe_interval: None,
        };
        PackageVideo {
            file_id: asset_file.id,
            task: PackageVideoTask::TranscodeVideo(video_target),
            repr_name,
            output_key: video_out_key,
        }
    });
    Ok(ghi_tasks
        .into_iter()
        .chain(video_tasks)
        .chain(audio_tasks)
        .collect())
}

/// Returns (existing_has_ghi, None) if GHI index already exists and there's nothing to do,
/// or (to_be_created_ghi, Some(..)) if GHI index needs to be created.
async fn check_create_ghi_task(
    conn: &mut PooledDbConn,
    file: &AssetFile,
    video: &Video,
) -> Result<(IsOriginalStreamable, Option<PackageVideo>)> {
    let file_id = video.file_id;

    if let Some(OriginalStreaming {
        is_streamable,
        is_done: true,
    }) = video.original_streaming
    {
        return Ok((is_streamable, None));
    };

    let acceptable_video_codecs = interact!(conn, move |conn| {
        repository::config::get_acceptable_video_codecs(conn)
    })
    .await??;
    let acceptable_audio_codecs = interact!(conn, move |conn| {
        repository::config::get_acceptable_audio_codecs(conn)
    })
    .await??;

    let orig_codec_ok = acceptable_video_codecs.contains(&video.video_codec_name);
    let orig_audio_codec_ok = video
        .audio_codec_name
        .as_ref()
        .is_none_or(|codec| acceptable_audio_codecs.contains(codec));
    let orig_audio_streamable = file.file_type == "mp4" && orig_audio_codec_ok;
    let orig_vid_streamable = file.file_type == "mp4" && orig_codec_ok;

    if (orig_vid_streamable || orig_audio_streamable)
        && let Some(max_iframe_interval) = video.max_iframe_interval
        && let Some((frame_rate_num, frame_rate_denom)) = video.frame_rate
        && frame_rate_denom > 0
        && frame_rate_num > 0
    {
        let is_streamable = match (orig_vid_streamable, orig_audio_streamable) {
            (true, true) => IsOriginalStreamable::VideoAudio,
            (true, false) => IsOriginalStreamable::VideoOnly,
            (false, true) => IsOriginalStreamable::AudioOnly,
            (false, false) => unreachable!(),
        };
        let task = if video.ghi_disabled {
            PackageVideoTask::DashSegment
        } else {
            interact!(conn, move |conn| {
                repository::asset::set_asset_original_streamable(
                    conn,
                    file_id,
                    OriginalStreaming {
                        is_streamable: is_streamable,
                        is_done: false,
                    },
                )
            })
            .await??;

            let gop_time = max_iframe_interval as f32 / 1000.0;
            let segment_duration = gop_time.clamp(1.0, 10.0).round() as i32;
            PackageVideoTask::CreateGHIIndex {
                include_video: orig_vid_streamable,
                include_audio: video.audio_codec_name.is_some() && orig_audio_streamable,
                segment_duration,
            }
        };

        Ok((
            is_streamable,
            Some(PackageVideo {
                file_id,
                repr_name: "original".to_owned(),
                task,
                output_key: storage_key::dash_file(file_id, format_args!("original")),
            }),
        ))
    } else {
        interact!(conn, move |conn| {
            repository::asset::set_asset_original_streamable(
                conn,
                file_id,
                OriginalStreaming {
                    is_streamable: IsOriginalStreamable::None,
                    is_done: false,
                },
            )
        })
        .await??;
        Ok((IsOriginalStreamable::None, None))
    }
}

pub async fn image_conversion_due(conn: &mut PooledDbConn) -> Result<Vec<ConvertImage>> {
    let acceptable_formats = ["jpeg", "avif", "png", "webp"];
    let assets_no_good_repr = interact!(conn, move |conn| {
        repository::asset::get_image_assets_with_no_acceptable_repr(conn, &acceptable_formats)
    })
    .await??;
    // there should be no duplicates
    debug_assert!(
        assets_no_good_repr.len()
            == assets_no_good_repr
                .clone()
                .into_iter()
                .collect::<HashSet<_>>()
                .len()
    );
    let ops = assets_no_good_repr
        .into_iter()
        .map(|file_id| {
            let target = ImageConversionTarget {
                scale: None,
                format: super::image_conversion_target::ImageFormatTarget::AVIF(
                    AvifTarget::default(),
                ),
            };
            let output_file_key = storage_key::image_representation(file_id, &target);
            ConvertImage {
                file_id,
                target,
                output_file_key,
            }
        })
        .collect();
    Ok(ops)
}
