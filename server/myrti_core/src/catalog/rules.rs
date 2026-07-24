use std::collections::HashSet;

use eyre::{Context, Result};
use itertools::Itertools;

use crate::{
    catalog::{
        encoding_target::{CodecTarget, VideoEncodingTarget, audio_codec_name, av1},
        operation::package_video::{AudioEncodingTarget, PackageVideoTask},
        storage_key,
    },
    config, interact,
    model::{
        AlbumId, AssetId, AssetThumbnail, FileId, ThumbnailFormat, ThumbnailType,
        repository::{self, asset::AssetHasThumbnails, db::PooledDbConn},
    },
};

use super::{
    image_conversion_target::{ImageConversionTarget, heif::AvifTarget},
    operation::{
        convert_image::ConvertImage,
        create_album_thumbnail::CreateAlbumThumbnail,
        create_thumbnail::{CreateAssetThumbnail, ThumbnailToCreate},
        package_video::PackageVideo,
    },
};

pub async fn required_video_packaging_for_asset(
    conn: &mut PooledDbConn,
    file_id: FileId,
    bin_paths: Option<&config::BinPaths>,
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

    let orig_codec_ok = acceptable_video_codecs.contains(&video.video_codec_name.as_str());
    let orig_audio_codec_ok = video
        .audio_codec_name
        .as_ref()
        .is_none_or(|codec| acceptable_audio_codecs.contains(&codec.as_str()));

    let has_ghi_index = interact!(
        conn,
        move |conn| repository::asset::get_asset_has_ghi_index(conn, file_id)
    )
    .await??;

    let mut ops = Vec::new();

    let orig_audio_streamable = video.is_original_streamable && orig_audio_codec_ok;
    let orig_vid_streamable = video.is_original_streamable && orig_codec_ok;

    if has_ghi_index.is_none() {
        if !orig_vid_streamable && !orig_audio_streamable {
            interact!(
                conn,
                move |conn| repository::asset::set_asset_has_ghi_index(conn, file_id, 0)
            )
            .await??;
        } else if let Some(max_iframe_interval) = video.max_iframe_interval
            && let Some((frame_rate_num, frame_rate_denom)) = video.frame_rate
            && frame_rate_denom > 0
            && frame_rate_num > 0
        {
            let gop_time =
                (max_iframe_interval as f32 / frame_rate_num as f32) * frame_rate_denom as f32;
            let segment_duration = gop_time.clamp(1.0, 10.0).round() as i32;
            ops.push(PackageVideo {
                file_id,
                repr_name: "original".to_owned(),
                task: PackageVideoTask::CreateGHIIndex {
                    include_video: orig_vid_streamable,
                    include_audio: video.audio_codec_name.is_some() && orig_audio_streamable,
                    segment_duration,
                },
                output_key: storage_key::dash_file(file_id, format_args!("original")),
            });
        }
    }

    if !orig_vid_streamable && !has_acceptable_video_repr {
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
    if !orig_audio_streamable && !orig_vid_streamable {
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
    let (image, file) = interact!(conn, move |conn| {
        repository::asset::get_image_file(conn, file_id)
    })
    .await??;
    let existing_image_reprs = interact!(conn, move |conn| {
        repository::representation::get_image_representations(conn, file_id)
    })
    .await??;

    let acceptable_formats = ["jpeg", "avif", "png", "webp"];
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

pub async fn required_thumbnails_for_asset(
    conn: &mut PooledDbConn,
    file_id: FileId,
) -> Result<CreateAssetThumbnail> {
    let have_thumbnails = interact!(conn, move |conn| {
        repository::asset::get_thumbnails_for_asset(conn, file_id)
    })
    .await??;
    Ok(CreateAssetThumbnail {
        file_id,
        thumbnails: missing_asset_thumbnails(have_thumbnails),
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
             }| CreateAssetThumbnail {
                file_id,
                thumbnails: missing_asset_thumbnails(thumbnails),
            },
        )
        .collect())
}

fn missing_asset_thumbnails(have_thumbnails: Vec<AssetThumbnail>) -> Vec<ThumbnailToCreate> {
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
    let missing_lg_orig: Vec<_> = want_formats
        .difference(&have_lg_orig_formats)
        .copied()
        .collect();
    let missing_sm_sq: Vec<_> = want_formats
        .difference(&have_sm_sq_formats)
        .copied()
        .collect();
    let mut missing = Vec::new();
    if !missing_lg_orig.is_empty() {
        missing.push(ThumbnailToCreate {
            ty: ThumbnailType::LargeOrigAspect,
            formats: missing_lg_orig,
        })
    }
    if !missing_sm_sq.is_empty() {
        missing.push(ThumbnailToCreate {
            ty: ThumbnailType::SmallSquare,
            formats: missing_sm_sq,
        })
    }
    missing
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

    // let acceptable_codecs_no_dash = interact!(conn, move |conn| {
    //     repository::asset::get_videos_in_acceptable_codec_without_dash(conn)
    // })
    // .await??;
    // let mut acceptable_codecs_no_dash_and_no_rotation_metadata: Vec<VideoAsset> = Vec::default();
    // for asset in acceptable_codecs_no_dash {
    // TODO: check iframe interval
    // let ffprobe_output = interact!(conn, move |conn| repository::asset::get_ffprobe_output(
    //     conn,
    //     asset.base.id
    // ))
    // .await??;
    // let streams = processing::video::ffprobe_get_streams_from_json(&ffprobe_output)
    //     .wrap_err("failed to parse ffprobe output stored in db")?;
    // acceptable_codecs_no_dash_and_no_rotation_metadata.push(asset);
    // }
    // let package_orig_tasks = std::iter::empty();
    // let package_orig_tasks = acceptable_codecs_no_dash_and_no_rotation_metadata
    //     .into_iter()
    //     .map(|asset| {
    //         // if there is no dash resource directory then there can not already be an audio
    //         // representation, so create one if the video has audio
    //         let create_audio_repr = if asset.video.audio_codec_name.is_some() {
    //             Some(CreateAudioRepr::PackageOriginalFile {
    //                 output_key: storage_key::dash_file(asset.base.id, format_args!("audio.mp4")),
    //             })
    //         } else {
    //             // video does not have audio
    //             None
    //         };
    //         // likewise for video representations
    //         PackageVideo {
    //             asset_id: asset.base.id,
    //             create_video_repr: CreateVideoRepr::PackageOriginalFile {
    //                 output_key: video_out_key,
    //             },
    //             create_audio_repr,
    //             create_ghi: None,
    //         }
    //     });

    // let no_good_reprs: Vec<VideoAsset> = interact!(conn, move |conn| {
    //     repository::asset::get_video_assets_with_no_acceptable_repr(conn)
    // })
    // .await??;
    return Ok(Default::default());
    // let reencode_tasks = no_good_reprs.into_iter().map(|asset| {
    //     let video_out_key = storage_key::dash_file(
    //         asset.base.id,
    //         format_args!("{}x{}.mp4", asset.base.size.width, asset.base.size.height),
    //     );
    //     let create_video_repr = match asset.video.video_codec_name.as_str() {
    //         // TODO replace with target codec from config and only transcode if the
    //         // original codec is unacceptable. Or maybe transcode anyway, provide a config
    //         // option etc..
    //         "av1" => CreateVideoRepr::PackageOriginalFile {
    //             output_key: video_out_key,
    //         },
    //         _ => CreateVideoRepr::Transcode(VideoTranscode {
    //             target: VideoEncodingTarget {
    //                 codec: CodecTarget::AV1(av1::AV1Target {
    //                     crf: av1::Crf::default(),
    //                     fast_decode: None,
    //                     preset: None,
    //                     max_bitrate: None,
    //                 }),
    //                 scale: None,
    //                 force_keyframe_interval: None,
    //             },
    //             output_key: video_out_key,
    //         }),
    //     };
    //     let audio_out_key = storage_key::dash_file(asset.base.id, format_args!("audio.mp4"));
    //     // TODO actually check existing reprs in database.
    //     // maybe the acceptable video in config changed, making us reencode video
    //     // but actually a suitable audio repr already exists (or vice versa)
    //     let create_audio_repr = match asset.video.audio_codec_name.as_deref() {
    //         Some("aac" | "opus" | "mp3") => Some(CreateAudioRepr::PackageOriginalFile {
    //             output_key: audio_out_key,
    //         }),
    //         // TODO matching strings is ehh since we only allow a few codecs anyway
    //         Some(_) => Some(CreateAudioRepr::Transcode(AudioTranscode {
    //             target: AudioEncodingTarget::OPUS,
    //             output_key: audio_out_key,
    //         })),
    //         None => None,
    //     };
    //     PackageVideo {
    //         asset_id: asset.base.id,
    //         create_video_repr,
    //         create_audio_repr,
    //         create_ghi: None,
    //     }
    // });
    // Ok(package_orig_tasks.chain(reencode_tasks).collect())
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
