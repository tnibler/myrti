use std::path::PathBuf;
use std::process::Stdio;

use eyre::{eyre, Context, Result};
use tokio::process::Command;
use tracing::{instrument, warn, Instrument};

use crate::catalog::encoding_target::audio_codec_name;
use crate::processing::video::dash_package::{
    generate_mpd, shaka_package, RepresentationInput, RepresentationType,
};
use crate::processing::video::probe_video;
use crate::processing::video::transcode::{ProduceAudio, ProduceVideo};
use crate::{
    catalog::encoding_target::{codec_name, VideoEncodingTarget},
    model::{
        repository::{self, pool::DbPool},
        AssetId, AudioRepresentation, AudioRepresentationId, Size, Video, VideoAsset,
        VideoRepresentation, VideoRepresentationId,
    },
    processing::video::transcode::ffmpeg_command,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateAudioRepr {
    Existing(AudioRepresentation),
    Transcode(AudioTranscode),
    PackageOriginalFile(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateVideoRepr {
    Existing(VideoRepresentation),
    PackageOriginalFile(PathBuf),
    Transcode(VideoTranscode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// all paths are relative to the dash resource directory
pub struct PackageVideo {
    pub asset_id: AssetId,
    pub create_video_repr: CreateVideoRepr,
    pub create_audio_repr: CreateAudioRepr,
    pub existing_video_reprs: Vec<VideoRepresentation>,
    pub mpd_output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVideoWithPath {
    pub output_dir: PathBuf,
    pub package_video: PackageVideo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatedVideoRepr {
    /// absolute path
    PackagedOriginalFile(PathBuf),
    Transcode(VideoTranscodeResult),
    Existing(VideoRepresentation),
}

// Some things like the resulting size and bitrate of
// a video we don't actually know until ffmpeg is done.
// That information needs to be known to apply the operation
// to the database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedPackageVideo {
    pub asset_id: AssetId,
    pub output_dir: PathBuf,
    pub created_video_repr: CreatedVideoRepr,
    pub created_audio_repr: CreateAudioRepr,
    /// relative to output_dir
    pub mpd_output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTranscode {
    pub target: VideoEncodingTarget,
    /// path where the final transcoded and shaka remuxed video file should be
    /// relative to PackageVideo::output_dir
    pub output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTranscodeResult {
    pub target: VideoEncodingTarget,
    pub final_size: Size,
    pub bitrate: i64,
    /// absolute path
    pub output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioEncodingTarget {
    AAC,
    OPUS,
    FLAC,
    MP3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTranscode {
    pub target: AudioEncodingTarget,
    /// path where the final transcoded and shaka packaged audio file should be
    /// relative to PackageVideo::output_dir
    pub output: PathBuf,
}

#[instrument(skip(pool))]
pub async fn apply_package_video(pool: &DbPool, op: &CompletedPackageVideo) -> Result<()> {
    let asset: VideoAsset = repository::asset::get_asset(pool, op.asset_id)
        .await?
        .try_into()?;
    let mut tx = pool
        .begin()
        .await
        .wrap_err("could not begin db transaction")?;
    let asset = VideoAsset {
        video: Video {
            dash_resource_dir: Some(op.output_dir.clone()),
            ..asset.video
        },
        ..asset
    };
    repository::asset::update_asset(tx.as_mut(), &(&asset).into()).await?;
    match &op.created_audio_repr {
        CreateAudioRepr::Transcode(audio_transcode) => {
            let audio_representation = AudioRepresentation {
                id: AudioRepresentationId(0),
                asset_id: op.asset_id,
                codec_name: audio_codec_name(&audio_transcode.target),
                path: op.output_dir.join(&audio_transcode.output),
            };
            let _audio_representation_id = repository::representation::insert_audio_representation(
                tx.as_mut(),
                &audio_representation,
            )
            .await?;
        }
        CreateAudioRepr::PackageOriginalFile(audio_output) => {
            let audio_representation = AudioRepresentation {
                id: AudioRepresentationId(0),
                asset_id: op.asset_id,
                codec_name: asset.video.audio_codec_name, // TODO
                path: op.output_dir.join(audio_output),
            };
            let _audio_representation_id = repository::representation::insert_audio_representation(
                tx.as_mut(),
                &audio_representation,
            )
            .await?;
        }
        CreateAudioRepr::Existing(_) => {}
    }
    let created_video_represention = match &op.created_video_repr {
        CreatedVideoRepr::PackagedOriginalFile(video_output) => Some(VideoRepresentation {
            id: VideoRepresentationId(0),
            asset_id: asset.base.id,
            codec_name: asset.video.video_codec_name,
            width: asset.base.size.width,
            height: asset.base.size.height,
            bitrate: asset.video.video_bitrate,
            path: op.output_dir.join(video_output),
        }),
        CreatedVideoRepr::Transcode(transcode) => Some(VideoRepresentation {
            id: VideoRepresentationId(0),
            asset_id: asset.base.id,
            codec_name: codec_name(&transcode.target.codec),
            width: transcode.final_size.width,
            height: transcode.final_size.height,
            bitrate: transcode.bitrate,
            path: op.output_dir.join(&transcode.output),
        }),
        CreatedVideoRepr::Existing(_) => None,
    };
    if let Some(video_represention) = created_video_represention {
        let _representation_id = repository::representation::insert_video_representation(
            tx.as_mut(),
            &video_represention,
        )
        .await?;
    }
    tx.commit()
        .await
        .wrap_err("could not commit db transaction")?;
    Ok(())
}

#[instrument(skip(pool))]
pub async fn perform_side_effects_package_video(
    pool: &DbPool,
    op: &PackageVideoWithPath,
) -> Result<CompletedPackageVideo> {
    tokio::fs::create_dir_all(&op.output_dir)
        .await
        .wrap_err("could not create output directory")?;
    let package_video = &op.package_video;
    let output_dir = &op.output_dir;
    let asset_id = package_video.asset_id;
    let asset_path = repository::asset::get_asset_path_on_disk(pool, asset_id).await?;

    // if video_transcode or audio_transcode
    //    ffmpeg if video_transcode -c:v target... else -c:v copy
    //           if audio_transcode -c:a target... else -c:a copy
    //           temp_file
    //    shaka-package if video_transcode or video_package_original:
    //                      input=temp_file,stream=video,output=...
    //                  if audio_transcode or audio_package_original:
    //                      input_temp_file,stream=audio,output=...
    // else if video_package_original or audio_package_original
    //    shaka-package if video_package_original:
    //                      input=original,stream=video,output=...
    //                  if audio_package_original:
    //                      input=original,stream=audio,output=...
    // mpd_generator created or existing video repr
    //               created or exiting audio repr
    let ffmpeg_video_op: Option<ProduceVideo> = match package_video.create_video_repr.clone() {
        CreateVideoRepr::Transcode(video_transcode) => {
            Some(ProduceVideo::Transcode(video_transcode.target))
        }
        _ => None,
    };
    let ffmpeg_audio_op: Option<ProduceAudio> = match package_video.create_audio_repr.clone() {
        CreateAudioRepr::Transcode(audio_transcode) => {
            Some(ProduceAudio::Transcode(audio_transcode.target))
        }
        _ => None,
    };

    let (ffmpeg_out_path, _ffmpeg_temp_dir) =
        if ffmpeg_video_op.is_some() || ffmpeg_audio_op.is_some() {
            let temp_dir = tempfile::tempdir().wrap_err("error creating temp directory")?;
            let ffmpeg_out_path = temp_dir.path().join("out.mp4");
            let mut command = ffmpeg_command(
                &asset_path.path_on_disk(),
                &ffmpeg_out_path,
                ffmpeg_video_op.as_ref(),
                ffmpeg_audio_op.as_ref(),
            );
            let ffmpeg_result = command.spawn()?.wait().await?;
            if !ffmpeg_result.success() {
                return Err(eyre!("ffmpeg exited with an error"));
            }
            (Some(ffmpeg_out_path), Some(temp_dir))
        } else {
            (None, None)
        };

    let audio_path = match &package_video.create_audio_repr {
        CreateAudioRepr::Existing(audio_repr) => audio_repr.path.clone(),
        CreateAudioRepr::PackageOriginalFile(audio_output) => {
            let out_path = output_dir.join(audio_output);
            let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                path: asset_path.path_on_disk(),
                ty: RepresentationType::Audio,
                out_path: out_path.clone(),
            }];
            shaka_package(repr_inputs)
                .await
                .wrap_err("could not shaka package audio stream")?;
            out_path
        }
        CreateAudioRepr::Transcode(transcode) => {
            let ffmpeg_out_path = ffmpeg_out_path
                .as_ref()
                .expect("ffmpeg output file must be present if audio was transcoded");
            let out_path = output_dir.join(&transcode.output);
            let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                path: ffmpeg_out_path.clone(),
                ty: RepresentationType::Audio,
                out_path: out_path.clone(),
            }];
            shaka_package(repr_inputs)
                .await
                .wrap_err("could not shaka package audio stream")?;
            out_path
        }
    };

    let created_video_repr = match &package_video.create_video_repr {
        CreateVideoRepr::Existing(video_repr) => CreatedVideoRepr::Existing(video_repr.clone()),
        CreateVideoRepr::PackageOriginalFile(video_output) => {
            let out_path = output_dir.join(video_output);
            if let Some(dir) = out_path.parent() {
                tokio::fs::create_dir_all(dir)
                    .in_current_span()
                    .await
                    .wrap_err("could not create video output dir")?;
            }
            // shaka-packager discards some metadata, notable stream side data like
            // rotation.
            // To correct that, we have to rerun the shaka-packager output through ffmpeg
            // to set the rotation again if present.
            // BUT since shaka-packager also outputs a media_info file that we need,
            // the shaka-packager output file needs to be the same as the final ffmpeg
            // output.
            // TODO calling ffprobe yet again, ideally once is enough
            let ffprobe = probe_video(&asset_path.path_on_disk()).await?;
            if let Some(rotation) = ffprobe.rotation {
                let temp_dir = tempfile::tempdir().wrap_err("could not create temp dir")?;
                let ffmpeg_out_path = temp_dir.path().join("video.mp4");
                let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                    path: asset_path.path_on_disk(),
                    ty: RepresentationType::Video,
                    out_path: out_path.clone(),
                }];
                shaka_package(repr_inputs)
                    .in_current_span()
                    .await
                    .wrap_err("could not shaka package video stream")?;
                let mut command = Command::new("ffmpeg");
                command
                    .args(["-nostdin", "-y", "-display_rotation"])
                    .arg(rotation.to_string())
                    .arg("-i")
                    .arg(&out_path)
                    .arg("-metadata:s:v")
                    .arg(format!(r#"rotate="{}""#, rotation))
                    .args(["-c:v", "copy", "-c:a", "copy"])
                    .arg(&ffmpeg_out_path)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let result = command
                    .spawn()
                    .wrap_err("failed to call ffmpeg")?
                    .wait()
                    .in_current_span()
                    .await
                    .wrap_err("error waiting for ffmpeg")?;
                if !result.success() {
                    return Err(eyre!("ffmpeg exited with an error"));
                }
                tokio::fs::copy(&ffmpeg_out_path, &out_path)
                    .in_current_span()
                    .await
                    .wrap_err("error copying ffmpeg output to dash dir")?;
                CreatedVideoRepr::PackagedOriginalFile(out_path)
            } else {
                let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                    path: asset_path.path_on_disk(),
                    ty: RepresentationType::Video,
                    out_path: out_path.clone(),
                }];
                shaka_package(repr_inputs)
                    .in_current_span()
                    .await
                    .wrap_err("could not shaka package video stream")?;
                CreatedVideoRepr::PackagedOriginalFile(out_path)
            }
        }
        CreateVideoRepr::Transcode(transcode) => {
            let ffmpeg_out_path = ffmpeg_out_path
                .as_ref()
                .expect("ffmpeg output file must be present if video was transcoded");
            let output_path = output_dir.join(&transcode.output);
            let repr_inputs: &[RepresentationInput] = &[RepresentationInput {
                path: ffmpeg_out_path.clone(),
                ty: RepresentationType::Video,
                out_path: output_path.clone(),
            }];
            shaka_package(repr_inputs)
                .await
                .wrap_err("could not shaka package video stream")?;
            let probe = probe_video(&output_path).await?;
            CreatedVideoRepr::Transcode(VideoTranscodeResult {
                target: transcode.target.clone(),
                final_size: Size {
                    width: probe.width,
                    height: probe.height,
                },
                bitrate: probe.bitrate,
                output: output_path,
            })
        }
    };

    let mut media_infos_relative: Vec<PathBuf> = Vec::default();
    for video_repr in &package_video.existing_video_reprs {
        // shaka packager generates video1.mp4.media_info for output video1.mp4
        let mut path = video_repr.path.clone();
        let filename = match video_repr.path.file_name() {
            Some(s) => s.to_str().unwrap(),
            None => {
                warn!(
                    repr_path = %video_repr.path.display(),
                    "No media_info for video representation"
                );
                continue;
            }
        };
        path.set_file_name(format!("{}.media_info", filename));
        media_infos_relative.push(path);
    }
    let created_video_repr_path = match &created_video_repr {
        CreatedVideoRepr::PackagedOriginalFile(video_path) => video_path.as_path(),
        CreatedVideoRepr::Transcode(transcode_result) => transcode_result.output.as_path(),
        CreatedVideoRepr::Existing(video_repr) => video_repr.path.as_path(),
    };
    match created_video_repr_path.file_name() {
        Some(filename) => {
            let mut path = created_video_repr_path.to_owned();
            path.set_file_name(format!("{}.media_info", filename.to_str().unwrap()));
            // path is actually absolute here, but since joining an absolute path just overwrites
            // it's fine
            media_infos_relative.push(path);
        }
        None => {
            warn!(
                repr_path = %created_video_repr_path.display(),
                "No media_info for video representation"
            );
        }
    };
    match &audio_path.file_name() {
        Some(filename) => {
            media_infos_relative.push(PathBuf::from(format!(
                "{}.media_info",
                filename.to_str().unwrap()
            )));
        }
        None => {
            warn!(
                repr_path = %audio_path.display(),
                "No media_info for audio representation"
            );
        }
    };
    let media_infos_abs: Vec<PathBuf> = media_infos_relative
        .into_iter()
        .map(|path| output_dir.join(path))
        .collect();
    generate_mpd(
        &media_infos_abs,
        output_dir.join(&package_video.mpd_output).as_path(),
    )
    .await
    .wrap_err("could not generate mpd manifest")?;
    Ok(CompletedPackageVideo {
        asset_id,
        output_dir: op.output_dir.clone(),
        mpd_output: package_video.mpd_output.clone(),
        created_video_repr,
        created_audio_repr: package_video.create_audio_repr.clone(),
    })
}
