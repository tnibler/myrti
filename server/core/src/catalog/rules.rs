use std::collections::HashSet;

use eyre::{Context, Result};
use tracing::instrument;

use crate::{
    catalog::{
        encoding_target::{av1, CodecTarget, VideoEncodingTarget},
        operation::package_video::{
            AudioEncodingTarget, AudioTranscode, CreateAudioRepr, CreateVideoRepr, VideoTranscode,
        },
        storage_key,
    },
    interact,
    model::{
        repository::{self, db::PooledDbConn},
        AssetId, AssetThumbnails, ThumbnailType, VideoAsset,
    },
    processing,
};

use super::{
    image_conversion_target::{heif::AvifTarget, ImageConversionTarget},
    operation::{
        convert_image::ConvertImage,
        create_thumbnail::{CreateThumbnail, ThumbnailToCreate},
        package_video::PackageVideo,
    },
};

#[instrument(skip(conn), level = "trace")]
pub async fn required_thumbnails_for_asset(
    conn: &mut PooledDbConn,
    asset_id: AssetId,
) -> Result<CreateThumbnail> {
    let existing_thumbnails = interact!(conn, move |mut conn| {
        repository::asset::get_thumbnails_for_asset(&mut conn, asset_id)
    })
    .await??;
    let mut create_thumbs = Vec::default();
    if !existing_thumbnails.has_thumb_small_square {
        create_thumbs.push(ThumbnailToCreate {
            ty: ThumbnailType::SmallSquare,
        });
    }
    if !existing_thumbnails.has_thumb_large_orig {
        create_thumbs.push(ThumbnailToCreate {
            ty: ThumbnailType::LargeOrigAspect,
        });
    }
    Ok(CreateThumbnail {
        asset_id,
        thumbnails: create_thumbs,
    })
}

#[instrument(skip(conn), level = "trace")]
pub async fn required_video_packaging_for_asset(
    conn: &mut PooledDbConn,
    asset_id: AssetId,
) -> Result<Vec<PackageVideo>> {
    let acceptable_video_codecs = ["h264", "av1", "vp9"];
    let acceptable_audio_codecs = ["aac", "opus", "flac", "mp3"];
    // TODO yes we're clearing and putting the config back in every time lol
    interact!(conn, move |conn| {
        repository::config::set_acceptable_audio_codecs(conn, &acceptable_audio_codecs)?;
        repository::config::set_acceptable_video_codecs(conn, &acceptable_video_codecs)?;
        Ok(())
    })
    .await??;

    let asset = interact!(conn, move |mut conn| {
        repository::asset::get_asset(&mut conn, asset_id)
    })
    .await??;
    let video = match asset.sp {
        crate::model::AssetSpe::Image(_) => {
            return Ok(Default::default());
        }
        crate::model::AssetSpe::Video(video) => video,
    };
    let existing_video_reprs = interact!(conn, move |mut conn| {
        repository::representation::get_video_representations(&mut conn, asset_id)
    })
    .await??;
    let has_acceptable_video_repr = existing_video_reprs
        .iter()
        .any(|repr| acceptable_video_codecs.contains(&repr.codec_name.as_str()));
    let audio_reprs = interact!(conn, move |mut conn| {
        repository::representation::get_audio_representations(&mut conn, asset_id)
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

    let ffprobe_output = interact!(conn, move |mut conn| repository::asset::get_ffprobe_output(
        &mut conn, asset_id
    ))
    .await??;
    let streams = processing::video::ffprobe_get_streams_from_json(&ffprobe_output)
        .wrap_err("failed to parse ffprobe output stored in db")?;
    let has_rotation_metadata = match streams.video.rotation {
        None | Some(0) => false,
        // have to reencode, as shaka packager discards stream tags like rotation
        Some(_rot) => true,
    };
    let video_out_key = storage_key::dash_file(
        asset.base.id,
        format_args!("{}x{}.mp4", asset.base.size.width, asset.base.size.height),
    );
    let create_video_repr = match orig_codec_ok && !has_rotation_metadata {
        true => {
            // no need to reencode
            CreateVideoRepr::PackageOriginalFile {
                output_key: video_out_key,
            }
        }
        false => {
            // reencode
            CreateVideoRepr::Transcode(VideoTranscode {
                target: VideoEncodingTarget {
                    codec: CodecTarget::AV1(av1::AV1Target {
                        crf: av1::Crf::default(),
                        fast_decode: None,
                        preset: None,
                        max_bitrate: None,
                    }),
                    scale: None,
                },
                output_key: video_out_key,
            })
        }
    };
    let create_audio_repr = match has_acceptable_audio_repr {
        true => None,
        false => Some(CreateAudioRepr::PackageOriginalFile {
            output_key: storage_key::dash_file(asset.base.id, format_args!("audio.mp4")),
        }),
    };
    Ok(vec![PackageVideo {
        asset_id: asset.base.id,
        create_video_repr,
        create_audio_repr,
        existing_video_reprs,
        mpd_out_key: storage_key::mpd_manifest(asset.base.id),
    }])
}

#[instrument(skip(conn), level = "trace")]
pub async fn required_image_conversion_for_asset(
    conn: &mut PooledDbConn,
    asset_id: AssetId,
) -> Result<Vec<ConvertImage>> {
    // TODO
    // tracing::error!("TODO not implemented required_image_conversion");
    Ok(Default::default())
}

#[instrument(skip(conn), level = "debug")]
pub async fn thumbnails_to_create(conn: &mut PooledDbConn) -> Result<Vec<CreateThumbnail>> {
    // always create all thumbnails if any are missing for now
    let limit: Option<i64> = None;
    let assets: Vec<AssetThumbnails> = interact!(conn, move |mut conn| {
        repository::asset::get_assets_with_missing_thumbnail(&mut conn, limit)
            .wrap_err("could not query for Assets with missing thumbnails")
    })
    .await??;
    Ok(assets
        .into_iter()
        .map(|asset| CreateThumbnail {
            asset_id: asset.id,
            thumbnails: vec![
                ThumbnailToCreate {
                    ty: ThumbnailType::SmallSquare,
                },
                ThumbnailToCreate {
                    ty: ThumbnailType::LargeOrigAspect,
                },
            ],
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
        repository::config::set_acceptable_audio_codecs(conn, &acceptable_audio_codecs)?;
        repository::config::set_acceptable_video_codecs(conn, &acceptable_video_codecs)?;
        Ok(())
    })
    .await??;

    let acceptable_codecs_no_dash = interact!(conn, move |mut conn| {
        repository::asset::get_videos_in_acceptable_codec_without_dash(&mut conn)
    })
    .await??;
    let mut acceptable_codecs_no_dash_and_no_rotation_metadata: Vec<VideoAsset> = Vec::default();
    let mut must_reencode_because_rotation_metadata: Vec<VideoAsset> = Vec::default();
    for asset in acceptable_codecs_no_dash {
        let ffprobe_output = interact!(
            conn,
            move |mut conn| repository::asset::get_ffprobe_output(&mut conn, asset.base.id)
        )
        .await??;
        let streams = processing::video::ffprobe_get_streams_from_json(&ffprobe_output)
            .wrap_err("failed to parse ffprobe output stored in db")?;
        match streams.video.rotation {
            None | Some(0) => {
                acceptable_codecs_no_dash_and_no_rotation_metadata.push(asset);
            }
            // have to reencode, as shaka packager discards stream tags like rotation
            Some(_rot) => {
                must_reencode_because_rotation_metadata.push(asset);
            }
        }
    }
    if !acceptable_codecs_no_dash_and_no_rotation_metadata.is_empty() {
        // FIXME shaka into ffmpeg does not work!
        // adding stream rotation tag with ffmpeg messes up offsets in mpd manifest and playback
        // breaks.
        // segmenting without reencoding only works if there is no stream rotation metadata tag,
        // we'll need to reencode in that case
        return Ok(acceptable_codecs_no_dash_and_no_rotation_metadata
            .into_iter()
            .map(|asset| {
                // if there is no dash resource directory then there can not already be an audio
                // representation, so create one if the video has audio
                let create_audio_repr = if asset.video.audio_codec_name.is_some() {
                    Some(CreateAudioRepr::PackageOriginalFile {
                        output_key: storage_key::dash_file(
                            asset.base.id,
                            format_args!("audio.mp4"),
                        ),
                    })
                } else {
                    // video does not have audio
                    None
                };
                // likewise for video representations
                let existing_video_reprs = Vec::default();
                let video_out_key = storage_key::dash_file(
                    asset.base.id,
                    format_args!("{}x{}.mp4", asset.base.size.width, asset.base.size.height),
                );
                PackageVideo {
                    asset_id: asset.base.id,
                    create_video_repr: CreateVideoRepr::PackageOriginalFile {
                        output_key: video_out_key,
                    },
                    create_audio_repr,
                    existing_video_reprs,
                    mpd_out_key: storage_key::mpd_manifest(asset.base.id),
                }
            })
            .collect());
    }

    let no_good_reprs: Vec<VideoAsset> = interact!(conn, move |mut conn| {
        repository::asset::get_video_assets_with_no_acceptable_repr(&mut conn)
    })
    .await??;
    if !no_good_reprs.is_empty() || !must_reencode_because_rotation_metadata.is_empty() {
        return Ok(no_good_reprs
            .into_iter()
            .chain(must_reencode_because_rotation_metadata.into_iter())
            .map(|asset| {
                let video_out_key = storage_key::dash_file(
                    asset.base.id,
                    format_args!("{}x{}.mp4", asset.base.size.width, asset.base.size.height),
                );
                let create_video_repr = match asset.video.video_codec_name.as_str() {
                    // TODO replace with target codec from config and only transcode if the
                    // original codec is unacceptable. Or maybe transcode anyway, provide a config
                    // option etc..
                    "av1" => CreateVideoRepr::PackageOriginalFile {
                        output_key: video_out_key,
                    },
                    _ => CreateVideoRepr::Transcode(VideoTranscode {
                        target: VideoEncodingTarget {
                            codec: CodecTarget::AV1(av1::AV1Target {
                                crf: av1::Crf::default(),
                                fast_decode: None,
                                preset: None,
                                max_bitrate: None,
                            }),
                            scale: None,
                        },
                        output_key: video_out_key,
                    }),
                };
                let audio_out_key =
                    storage_key::dash_file(asset.base.id, format_args!("audio.mp4"));
                // TODO actually check existing reprs in database.
                // maybe the acceptable video in config changed, making us reencode video
                // but actually a suitable audio repr already exists (or vice versa)
                let create_audio_repr = match asset.video.audio_codec_name.as_deref() {
                    Some("aac" | "opus" | "mp3") => Some(CreateAudioRepr::PackageOriginalFile {
                        output_key: audio_out_key,
                    }),
                    // TODO matching strings is ehh since we only allow a few codecs anyway
                    Some(_) => Some(CreateAudioRepr::Transcode(AudioTranscode {
                        target: AudioEncodingTarget::OPUS,
                        output_key: audio_out_key,
                    })),
                    None => None,
                };
                PackageVideo {
                    asset_id: asset.base.id,
                    create_video_repr,
                    create_audio_repr,
                    existing_video_reprs: Vec::default(),
                    mpd_out_key: storage_key::mpd_manifest(asset.base.id),
                }
            })
            .collect());
    }
    // return Ok(vec![]);
    // if all videos have at least one good repr:
    //   for every rung in quality_levels starting from the highest
    //     for every video, ql = quality_ladder(video):
    //       if rung in ql and no repr for video at that rung:
    //         transcode to that rung
    // TODO
    Ok(Vec::default())
}

pub async fn image_conversion_due(conn: &mut PooledDbConn) -> Result<Vec<ConvertImage>> {
    let acceptable_formats = ["jpeg", "avif", "png", "webp"];
    let assets_no_good_repr = interact!(conn, move |mut conn| {
        repository::asset::get_image_assets_with_no_acceptable_repr(&mut conn, &acceptable_formats)
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
        .map(|asset_id| {
            let target = ImageConversionTarget {
                scale: None,
                format: super::image_conversion_target::ImageFormatTarget::AVIF(
                    AvifTarget::default(),
                ),
            };
            let output_file_key = storage_key::image_representation(asset_id, &target);
            ConvertImage {
                asset_id,
                target,
                output_file_key,
            }
        })
        .collect();
    Ok(ops)
}
