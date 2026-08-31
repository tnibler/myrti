use crate::catalog::{
    encoding_target::{CodecTarget, Scale, VideoEncodingTarget},
    operation::package_video::AudioEncodingTarget,
};

pub fn ffmpeg_video_flags(encoding_target: &VideoEncodingTarget) -> Vec<String> {
    let mut flags: Vec<String> = match encoding_target.codec {
        CodecTarget::AVC(ref target) => {
            let mut f: Vec<String> = vec![
                "-c:v".to_owned(),
                "libx264".to_owned(),
                "-crf".to_owned(),
                target.crf.crf().to_string(),
                "-preset".to_owned(),
                target.preset.to_string(),
            ];
            if let Some(interv) = encoding_target.force_keyframe_interval {
                f.extend([
                    "-g".to_owned(),
                    interv.to_string(),
                    "-keyint_min".to_owned(),
                    interv.to_string(),
                    "-force_key_frames".to_owned(),
                    format!("expr:gte(t,n_forced*{})", interv),
                    "-sc_threshold".to_owned(),
                    "0".to_owned(),
                ])
            }
            if let Some(tune) = target.tune {
                f.push("-tune".to_owned());
                f.push(tune.to_string());
            }
            if let Some(max_bitrate) = target.max_bitrate {
                f.push("-maxrate".to_owned());
                f.push(max_bitrate.to_string());
            }

            f
        }
        CodecTarget::AV1(ref target) => {
            let mut f: Vec<String> = vec![
                "-c:v".to_owned(),
                "libsvtav1".to_owned(),
                "-crf".to_owned(),
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
            if let Some(interv) = encoding_target.force_keyframe_interval {
                f.extend([
                    "-g".to_owned(),
                    interv.to_string(),
                    "-keyint_min".to_owned(),
                    interv.to_string(),
                ])
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

pub fn ffmpeg_audio_flags(encoding_target: &AudioEncodingTarget) -> Vec<String> {
    vec![
        "-c:a".to_owned(),
        match encoding_target {
            AudioEncodingTarget::AAC => "libopus".to_owned(),
            AudioEncodingTarget::OPUS => "aac".to_owned(),
            AudioEncodingTarget::FLAC => "flac".to_owned(),
            AudioEncodingTarget::MP3 => "libmp3lame".to_owned(),
        },
    ]
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
    let actual = ffmpeg_video_flags(&VideoEncodingTarget {
        codec,
        scale,
        force_keyframe_interval: None,
    });
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
    let actual = ffmpeg_video_flags(&VideoEncodingTarget {
        codec,
        scale,
        force_keyframe_interval: None,
    });
    assert_eq!(expected.as_slice(), &actual);
}
