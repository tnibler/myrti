use crate::catalog::{
    encoding_target::{CodecTarget, Scale, VideoEncodingTarget},
    operation::package_video::AudioEncodingTarget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProduceAudio {
    Copy,
    Transcode(AudioEncodingTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProduceVideo {
    Copy,
    Transcode(VideoEncodingTarget),
}

pub fn ffmpeg_video_flags(produce_video: &ProduceVideo) -> Vec<String> {
    match produce_video {
        ProduceVideo::Copy => vec![format!("-c:v"), format!("copy")],
        ProduceVideo::Transcode(encoding_target) => {
            let mut flags: Vec<String> = match encoding_target.codec {
                CodecTarget::AVC(ref target) => {
                    let mut f: Vec<String> = vec![
                        format!("-c:v"),
                        format!("libx264"),
                        format!("-crf"),
                        target.crf.crf().to_string(),
                        format!("-preset"),
                        target.preset.to_string(),
                    ];
                    if let Some(tune) = target.tune {
                        f.push("-tune".to_string());
                        f.push(tune.to_string());
                    }
                    if let Some(max_bitrate) = target.max_bitrate {
                        f.push("-maxrate".to_string());
                        f.push(max_bitrate.to_string());
                    }

                    f
                }
                CodecTarget::AV1(ref target) => {
                    let mut f: Vec<String> = vec![
                        format!("-c:v"),
                        format!("libsvtav1"),
                        format!("-crf"),
                        target.crf.crf().to_string(),
                    ];
                    if let Some(preset) = target.preset {
                        f.push("-preset".to_string());
                        f.push(preset.preset().to_string());
                    }
                    if let Some(max_bitrate) = target.max_bitrate {
                        f.push("-maxrate".to_string());
                        f.push(max_bitrate.to_string());
                    }
                    if let Some(fast_decode) = target.fast_decode {
                        f.push("-svtav1-params".to_string());
                        f.push(format!("fast-decode={}", fast_decode.fast_decode()));
                    }
                    f
                }
            };
            if let Some(scale) = encoding_target.scale {
                let scale_multiple: i32 = match encoding_target.codec {
                    CodecTarget::AVC(_) => 2,
                    CodecTarget::AV1(_) => 2,
                };
                flags.push("-vf".to_string());
                let scale_str = match scale {
                    Scale::HeightKeepAspect { height } => format!("-{}:{}", scale_multiple, height),
                    Scale::WidthKeepAspect { width } => format!("{}:-{}", width, scale_multiple),
                };
                flags.push(format!("scale={}", scale_str));
            }
            flags
        }
    }
}

pub fn ffmpeg_audio_flags(produce_audio: &ProduceAudio) -> Vec<String> {
    match produce_audio {
        ProduceAudio::Copy => vec![format!("-c:a"), format!("copy")],
        ProduceAudio::Transcode(encoding_target) => vec![
            format!("-c:a"),
            match encoding_target {
                AudioEncodingTarget::AAC => "libopus".to_string(),
                AudioEncodingTarget::OPUS => "aac".to_string(),
                AudioEncodingTarget::FLAC => "flac".to_string(),
                AudioEncodingTarget::MP3 => "libmp3lame".to_string(),
            },
        ],
    }
}

#[test]
fn ffmpeg_avc_flags_assembled_correctly() {
    use crate::catalog::encoding_target::avc::*;
    let codec = CodecTarget::AVC(AVCTarget {
        preset: Preset::Medium,
        tune: Some(Tune::Zerolatency),
        crf: Crf::try_from(24).unwrap(),
        max_bitrate: Some(10_000_000),
    });
    let scale = Some(Scale::WidthKeepAspect { width: 1280 });
    let expected = [
        "-c:v",
        "libx264",
        "-crf",
        "24",
        "-preset",
        "medium",
        "-tune",
        "zerolatency",
        "-maxrate",
        "10000000",
        "-vf",
        "scale=1280:-2",
    ];
    let actual = ffmpeg_video_flags(&ProduceVideo::Transcode(VideoEncodingTarget {
        codec,
        scale,
    }));
    assert_eq!(expected.as_slice(), &actual);
}

#[test]
fn ffmpeg_av1_command_assembled_correctly() {
    use crate::catalog::encoding_target::av1::*;
    let codec = CodecTarget::AV1(AV1Target {
        preset: Some(Preset::try_from(8).unwrap()),
        crf: Crf::try_from(45).unwrap(),
        max_bitrate: Some(4_000_000),
        fast_decode: Some(FastDecode::try_from(1).unwrap()),
    });
    let scale: Option<Scale> = Some(Scale::HeightKeepAspect { height: 500 });
    let expected = [
        "-c:v",
        "libsvtav1",
        "-crf",
        "45",
        "-preset",
        "8",
        "-maxrate",
        "4000000",
        "-svtav1-params",
        "fast-decode=1",
        "-vf",
        "scale=-2:500",
    ];
    let actual = ffmpeg_video_flags(&ProduceVideo::Transcode(VideoEncodingTarget {
        codec,
        scale,
    }));
    assert_eq!(expected.as_slice(), &actual);
}
