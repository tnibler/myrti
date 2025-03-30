use std::{ffi::OsString, process::Stdio};

use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::Result;
use tokio::process::Command;

use crate::{
    catalog::{
        encoding_target::{
            audio_codec_name, av1::AV1Target, avc::AVCTarget, codec_name, CodecTarget,
            VideoEncodingTarget,
        },
        image_conversion_target::{heif::AvifTarget, jpeg::JpegTarget},
        operation::package_video::AudioEncodingTarget,
    },
    config::BinPaths,
    util::OptionPathExt,
};

use super::video::transcode::{ffmpeg_audio_flags, ffmpeg_video_flags, ProduceAudio, ProduceVideo};

pub async fn run_self_check(bin_paths: Option<&BinPaths>) -> Result<(), ()> {
    let ffmpeg_bin_path: Option<&Path> = bin_paths.and_then(|bp| bp.ffmpeg.as_opt_path());
    check_can_run_ffmpeg(ffmpeg_bin_path).await?;
    check_can_encode_video(ffmpeg_bin_path).await?;
    check_can_encode_audio(ffmpeg_bin_path).await?;
    let shaka_bin_path: Option<&Path> = bin_paths.and_then(|bp| bp.shaka_packager.as_opt_path());
    let mpd_generator_bin_path: Option<&Path> =
        bin_paths.and_then(|bp| bp.mpd_generator.as_opt_path());
    check_can_run_shaka_and_mpd_generator(shaka_bin_path, mpd_generator_bin_path).await?;
    let exiftool_bin_path: Option<&Path> = bin_paths.and_then(|bp| bp.exiftool.as_opt_path());
    check_can_run_exiftool(exiftool_bin_path).await?;
    check_can_encode_vips_images().await?;
    Ok(())
}

async fn check_can_run_ffmpeg(ffmpeg_bin_path: Option<&Path>) -> Result<(), ()> {
    let spawn_result = Command::new(ffmpeg_bin_path.map(|p| p.as_str()).unwrap_or("ffmpeg"))
        .arg("-version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let ffmpeg = match spawn_result {
        Ok(c) => c,
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                tracing::error!("Could not find ffmpeg. Is it installed?");
                return Err(());
            }
            _kind => {
                tracing::error!("Error running ffmpeg: {}", err);
                return Err(());
            }
        },
    };
    let output = match ffmpeg.wait_with_output().await {
        Ok(o) => o,
        Err(err) => {
            tracing::error!(
                "ffmpeg test failed, error waiting for ffmpeg process: {}",
                err
            );
            return Err(());
        }
    };
    if !output.status.success() {
        tracing::error!(
            "ffmpeg test failed, error running ffmpeg:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        return Err(());
    }
    tracing::debug!("ok: can run ffmpeg");
    Ok(())
}

async fn check_can_encode_video(ffmpeg_bin_path: Option<&Path>) -> Result<(), ()> {
    let encoding_targets = [
        VideoEncodingTarget {
            codec: CodecTarget::AVC(AVCTarget::default()),
            scale: None,
        },
        VideoEncodingTarget {
            codec: CodecTarget::AV1(AV1Target::default()),
            scale: None,
        },
    ];
    let input = "color=white:640x480:duration=3";
    let pre_input_flags: Vec<OsString> = ["-loglevel", "warning", "-f", "lavfi"]
        .into_iter()
        .map(|s| s.into())
        .collect();
    for encoding_target in encoding_targets {
        let name = codec_name(&encoding_target.codec);
        let video_flags: Vec<OsString> =
            ffmpeg_video_flags(&ProduceVideo::Transcode(encoding_target))
                .into_iter()
                .map(|s| s.into())
                .collect();
        let out_path: PathBuf = format!("/tmp/_myrti_test_{}.mp4", name).into();

        let mut command = Command::new(ffmpeg_bin_path.unwrap_or("ffmpeg".into()));
        command
            .arg("-nostdin")
            .arg("-y")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.args(pre_input_flags.iter());
        command.arg("-i").arg(input);
        command.args(video_flags.iter());
        command.arg(&out_path);
        let ffmpeg = match command.spawn() {
            Ok(c) => c,
            Err(err) => {
                tracing::error!("Error running ffmpeg: {}", err);
                return Err(());
            }
        };
        let output = match ffmpeg.wait_with_output().await {
            Ok(o) => o,
            Err(err) => {
                tracing::error!(
                    "ffmpeg test failed, error waiting for ffmpeg process: {}",
                    err
                );
                return Err(());
            }
        };
        if !output.status.success() {
            tracing::error!(
                "Error producing test {} file at {} with ffmpeg\nCommand:\n{:?}\nffmpeg output:\n{}",
                name,
                out_path,
                command.as_std(),
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(());
        }
        tracing::debug!("ok: can encode {} video", name);
    }
    Ok(())
}

async fn check_can_encode_audio(ffmpeg_bin_path: Option<&Path>) -> Result<(), ()> {
    let encoding_targets = [
        AudioEncodingTarget::AAC,
        AudioEncodingTarget::OPUS,
        AudioEncodingTarget::MP3,
        AudioEncodingTarget::FLAC,
    ];
    let input = "sine=frequency=500:duration=3";
    let pre_input_flags: Vec<OsString> = ["-loglevel", "warning", "-f", "lavfi"]
        .into_iter()
        .map(|s| s.into())
        .collect();
    for encoding_target in encoding_targets {
        let name = audio_codec_name(&encoding_target);
        let audio_flags: Vec<OsString> =
            ffmpeg_audio_flags(&ProduceAudio::Transcode(encoding_target))
                .into_iter()
                .map(|s| s.into())
                .collect();
        let out_path: PathBuf = format!("/tmp/_myrti_test_{}.mp4", name).into();

        let mut command = Command::new(ffmpeg_bin_path.unwrap_or("ffmpeg".into()));
        command
            .arg("-nostdin")
            .arg("-y")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.args(pre_input_flags.iter());
        command.arg("-i").arg(input);
        command.args(audio_flags.iter());
        command.arg(&out_path);
        let ffmpeg = match command.spawn() {
            Ok(c) => c,
            Err(err) => {
                tracing::error!("Error running ffmpeg: {}", err);
                return Err(());
            }
        };
        let output = match ffmpeg.wait_with_output().await {
            Ok(o) => o,
            Err(err) => {
                tracing::error!(
                    "ffmpeg test failed, error waiting for ffmpeg process: {}",
                    err
                );
                return Err(());
            }
        };
        if !output.status.success() {
            tracing::error!(
                "Error producing test {} file at {} with ffmpeg\nCommand:\n{:?}\nffmpeg output:\n{}",
                name,
                out_path,
                command.as_std(),
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(());
        }
        tracing::debug!("ok: can encode {} audio", name);
    }
    Ok(())
}

async fn check_can_run_shaka_and_mpd_generator(
    shaka_bin_path: Option<&Path>,
    mpd_generator_bin_path: Option<&Path>,
) -> Result<(), ()> {
    let mut shaka_command = Command::new(shaka_bin_path.map(|p| p.as_str()).unwrap_or("packager"));
    shaka_command.stdout(Stdio::piped());
    shaka_command.stderr(Stdio::piped());
    shaka_command.arg("--version");
    let shaka_proc = match shaka_command.spawn() {
        Ok(c) => c,
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                if let Some(shaka_path) = mpd_generator_bin_path {
                    tracing::error!(
                        "Could not find shaka packager at path from config: {}",
                        shaka_path
                    );
                } else {
                    tracing::error!(
                        "Could not find shaka packager (no 'packager' in $PATH). Please download it and specify its location in the config file",
                    );
                }
                return Err(());
            }
            _kind => {
                tracing::error!("Error running shaka packager: {}", err);
                return Err(());
            }
        },
    };
    let shaka_result = shaka_proc.wait_with_output().await;
    match shaka_result {
        Ok(result) => {
            if !result.status.success() {
                tracing::error!(
                    "Testing shaka packager with 'packager --version' exited with an error:\n{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                return Err(());
            }
        }
        Err(err) => {
            tracing::error!(
                "Testing shaka packager with 'packager --version' failed:\n{:?}",
                err
            );
            return Err(());
        }
    }
    tracing::debug!("ok: can run shaka packager");

    let mut mpdg_command = Command::new(
        mpd_generator_bin_path
            .map(|p| p.as_str())
            .unwrap_or("mpd_generator"),
    );
    mpdg_command.stdout(Stdio::piped());
    mpdg_command.stderr(Stdio::piped());
    mpdg_command.arg("--version");
    let mpdg_proc = match mpdg_command.spawn() {
        Ok(c) => c,
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                if let Some(mpdg_path) = mpd_generator_bin_path {
                    tracing::error!(
                        "Could not find mpd_generator at path from config: {}",
                        mpdg_path
                    );
                } else {
                    tracing::error!(
                        "Could not find mpd_generator (no 'mpd_generator ' in $PATH). Please download it and specify its location in the config file",
                    );
                }
                return Err(());
            }
            _kind => {
                tracing::error!("Error running mpd_generator: {}", err);
                return Err(());
            }
        },
    };
    let mpdg_result = mpdg_proc.wait_with_output().await;
    match mpdg_result {
        Ok(result) => {
            if !result.status.success() {
                tracing::error!(
                    "Testing mpd_generator with 'mpd_generator  --version' exited with an error:\n{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                return Err(());
            }
        }
        Err(err) => {
            tracing::error!(
                "Testing mpd_generator  with 'mpd_generator  --version' failed:\n{:?}",
                err
            );
            return Err(());
        }
    }
    tracing::debug!("ok: can run mpd_generator");
    Ok(())
}

async fn check_can_encode_vips_images() -> Result<(), ()> {
    // TODO this should run in rayon
    // TODO test webp as well
    let jpeg_result = crate::processing::image::save_test_jpeg_image(
        "/tmp/__myrti_test.jpg".into(),
        &JpegTarget {
            quality: 90.try_into().expect("is valid quality factor"),
        },
    );
    match jpeg_result {
        Ok(_) => {
            tracing::debug!("ok: can encode JPEG image");
        }
        Err(_err) => {
            tracing::error!("Error saving test JPEG image");
            return Err(());
        }
    }
    let avif_result = crate::processing::image::save_test_heif_image(
        "/tmp/_myrti_test.avif".into(),
        &AvifTarget::default(),
    );
    match avif_result {
        Ok(_) => {
            tracing::debug!("ok: can encode AVIF image");
        }
        Err(_err) => {
            tracing::error!("Error saving test AVIF image");
            return Err(());
        }
    }

    let avif_result =
        crate::processing::image::save_test_webp_image("/tmp/_myrti_test.webp".into());
    match avif_result {
        Ok(_) => {
            tracing::debug!("ok: can encode WEBP image");
        }
        Err(_err) => {
            tracing::error!("Error saving test WEBP image");
            return Err(());
        }
    }
    Ok(())
}

async fn check_can_run_exiftool(exiftool_bin_path: Option<&Path>) -> Result<(), ()> {
    let spawn_result = Command::new(exiftool_bin_path.map(|p| p.as_str()).unwrap_or("exiftool"))
        .arg("-ver")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let exift_proc = match spawn_result {
        Ok(c) => c,
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                tracing::error!("Could not find exiftool. Is it installed?");
                return Err(());
            }
            _kind => {
                tracing::error!("Error running exiftool: {}", err);
                return Err(());
            }
        },
    };
    let output = match exift_proc.wait_with_output().await {
        Ok(o) => o,
        Err(err) => {
            tracing::error!(
                "exiftool test failed, error waiting for exiftool process: {}",
                err
            );
            return Err(());
        }
    };
    if !output.status.success() {
        tracing::error!(
            "exiftool test failed, error running exiftool:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(());
    }
    tracing::debug!("ok: can run exifool");
    Ok(())
}
