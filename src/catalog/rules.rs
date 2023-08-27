use std::path::PathBuf;

use eyre::{Context, Result};
use tracing::instrument;

use crate::{
    catalog::{
        encoding_target::{CodecTarget, VideoEncodingTarget},
        operation::package_video::{
            AudioEncodingTarget, AudioTranscode, CreateAudioRepr, CreateVideoRepr, VideoTranscode,
        },
    },
    model::{
        repository::{self, pool::DbPool},
        AssetThumbnails, ThumbnailType, VideoAsset,
    },
};

use super::operation::{
    create_thumbnail::{CreateThumbnail, ThumbnailToCreate},
    package_video::PackageVideo,
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
                let video_out_path = PathBuf::from(asset.video.video_codec_name).join(format!(
                    "{}x{}.mp4",
                    asset.base.size.width, asset.base.size.height
                ));
                // if there is no dash resource directory then there can not already be an audio
                // representation
                let create_audio_repr =
                    CreateAudioRepr::PackageOriginalFile(PathBuf::from("audio.mp4"));
                // likewise for video representations
                let existing_video_reprs = Vec::default();
                PackageVideo {
                    asset_id: asset.base.id,
                    create_video_repr: CreateVideoRepr::PackageOriginalFile(video_out_path),
                    create_audio_repr,
                    existing_video_reprs,
                    mpd_output: PathBuf::from("stream.mpd"),
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
    if !no_good_reprs.is_empty() {
        use crate::catalog::encoding_target::av1;
        return Ok(no_good_reprs
            .into_iter()
            .map(|asset| {
                let video_output: PathBuf = format!(
                    "av1/{}x{}.mp4",
                    asset.base.size.width, asset.base.size.height
                )
                .into();
                let create_video_repr = match asset.video.video_codec_name.as_str() {
                    // TODO replace with target codec from config
                    "av1" => CreateVideoRepr::PackageOriginalFile(video_output),
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
                        output: video_output,
                    }),
                };
                let audio_output: PathBuf = "audio.mp4".into();
                // TODO actually check existing reprs in database.
                // maybe the acceptable video in config changed, making us reencode video
                // but actually a suitable audio repr already exists (or vice versa)
                let create_audio_repr = match asset.video.audio_codec_name.as_str() {
                    "aac" | "opus" | "mp3" => CreateAudioRepr::PackageOriginalFile(audio_output),
                    _ => CreateAudioRepr::Transcode(AudioTranscode {
                        target: AudioEncodingTarget::OPUS,
                        output: audio_output,
                    }),
                };
                PackageVideo {
                    asset_id: asset.base.id,
                    mpd_output: PathBuf::from("stream.mpd"),
                    create_video_repr,
                    create_audio_repr,
                    existing_video_reprs: Vec::default(),
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
