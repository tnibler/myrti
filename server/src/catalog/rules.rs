use std::collections::HashSet;

use eyre::{Context, Result};
use tracing::instrument;

use crate::{
    catalog::{
        encoding_target::{CodecTarget, VideoEncodingTarget},
        operation::package_video::{
            AudioEncodingTarget, AudioTranscode, CreateAudioRepr, CreateVideoRepr, VideoTranscode,
        },
        storage_key,
    },
    model::{
        repository::{self, pool::DbPool},
        AssetThumbnails, ThumbnailType, VideoAsset,
    },
};

use super::{
    image_conversion_target::{heif::AvifTarget, ImageConversionTarget},
    operation::{
        convert_image::ConvertImage,
        create_thumbnail::{CreateThumbnail, ThumbnailToCreate},
        package_video::PackageVideo,
    },
};

#[instrument(skip(pool))]
pub async fn thumbnails_to_create(pool: &DbPool) -> Result<Vec<CreateThumbnail>> {
    // always create all thumbnails if any are missing for now
    let limit: Option<i32> = None;
    let assets: Vec<AssetThumbnails> =
        repository::asset::get_assets_with_missing_thumbnail(pool, limit)
            .await
            .wrap_err("could not query for Assets with missing thumbnails")?;
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

pub async fn video_packaging_due(pool: &DbPool) -> Result<Vec<PackageVideo>> {
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
    let acceptable_codecs_no_dash = repository::asset::get_videos_in_acceptable_codec_without_dash(
        pool,
        acceptable_video_codecs.into_iter(),
        acceptable_audio_codecs.into_iter(),
    )
    .await?;
    if !acceptable_codecs_no_dash.is_empty() {
        return Ok(acceptable_codecs_no_dash
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

    let no_good_reprs: Vec<VideoAsset> =
        repository::asset::get_video_assets_with_no_acceptable_repr(
            pool,
            acceptable_video_codecs.into_iter(),
            acceptable_audio_codecs.into_iter(),
        )
        .await
        .unwrap();
    tracing::info!(?no_good_reprs, "videos with no good reprs");
    if !no_good_reprs.is_empty() {
        use crate::catalog::encoding_target::av1;
        return Ok(no_good_reprs
            .into_iter()
            .map(|asset| {
                let video_out_key = storage_key::dash_file(
                    asset.base.id,
                    format_args!("{}x{}.mp4", asset.base.size.width, asset.base.size.height),
                );
                let create_video_repr = match asset.video.video_codec_name.as_str() {
                    // TODO replace with target codec from config
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
    todo!()
}

pub async fn image_conversion_due(pool: &DbPool) -> Result<Vec<ConvertImage>> {
    let acceptable_formats = ["jpeg", "avif", "png", "webp"];
    let assets_no_good_repr = repository::asset::get_image_assets_with_no_acceptable_repr(
        pool,
        acceptable_formats.into_iter(),
    )
    .await?;
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
            ConvertImage {
                asset_id,
                output_key: storage_key::image_representation(asset_id, &target),
                target,
            }
        })
        .collect();
    Ok(ops)
}
