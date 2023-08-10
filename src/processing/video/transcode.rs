use std::path::{Path, PathBuf};

use eyre::Result;
use tokio::process::Command;

mod avc {
    use eyre::{eyre, Report};
    use strum_macros::Display;

    #[derive(Debug, Clone, Display, Copy)]
    pub enum Preset {
        #[strum(serialize = "ultrafast")]
        Ultrafast,
        #[strum(serialize = "superfast")]
        Superfast,
        #[strum(serialize = "veryfast")]
        Veryfast,
        #[strum(serialize = "faster")]
        Faster,
        #[strum(serialize = "fast")]
        Fast,
        #[strum(serialize = "medium")]
        Medium,
        #[strum(serialize = "slow")]
        Slow,
        #[strum(serialize = "slower")]
        Slower,
        #[strum(serialize = "veryslow")]
        Veryslow,
    }

    impl Default for Preset {
        fn default() -> Self {
            Self::Medium
        }
    }

    #[derive(Debug, Clone, Display, Copy)]
    pub enum Tune {
        #[strum(serialize = "film")]
        Film,
        #[strum(serialize = "animation")]
        Animation,
        #[strum(serialize = "grain")]
        Grain,
        #[strum(serialize = "stillimage")]
        Stillimage,
        #[strum(serialize = "fastdecode")]
        Fastdecode,
        #[strum(serialize = "zerolatency")]
        Zerolatency,
    }

    /// https://trac.ffmpeg.org/wiki/Encode/H.264
    /// The range of the CRF scale is 0–51, where 0 is lossless (for 8 bit only, for 10 bit use -qp 0), 23 is the default, and 51 is worst quality possible.
    /// A lower value generally leads to higher quality, and a subjectively sane range is 17–28.
    /// Consider 17 or 18 to be visually lossless or nearly so; it should look the same or nearly the same as the input but it isn't technically lossless.
    /// The range is exponential, so increasing the CRF value +6 results in roughly half the bitrate / file size, while -6 leads to roughly twice the bitrate.
    #[derive(Debug, Clone, Copy)]
    pub struct Crf {
        crf: i32,
    }

    impl Crf {
        pub fn crf(&self) -> i32 {
            self.crf
        }
    }

    impl TryFrom<i32> for Crf {
        type Error = Report;

        fn try_from(value: i32) -> Result<Self, Self::Error> {
            match value {
                0..=51 => Ok(Crf { crf: value }),
                _ => Err(eyre!("invalid x264 CRF value {}", value)),
            }
        }
    }

    impl Default for Crf {
        fn default() -> Self {
            Self { crf: 23 }
        }
    }

    #[derive(Debug, Clone)]
    pub struct AVCTarget {
        pub preset: Preset,
        pub tune: Option<Tune>,
        pub crf: Crf,
        pub max_bitrate: Option<u32>,
    }
}

mod av1 {
    use eyre::{eyre, Report};

    /// https://trac.ffmpeg.org/wiki/Encode/AV1#CRF
    /// The valid CRF value range is 0-63, with the default being 50.
    /// Lower values correspond to higher quality and greater file size.
    #[derive(Debug, Clone, Copy)]
    pub struct Crf {
        crf: i32,
    }

    impl Crf {
        pub fn crf(&self) -> i32 {
            self.crf
        }
    }

    impl TryFrom<i32> for Crf {
        type Error = Report;

        fn try_from(value: i32) -> Result<Self, Self::Error> {
            match value {
                0..=63 => Ok(Crf { crf: value }),
                _ => Err(eyre!("invalid SVT-AV1 CRF value {}", value)),
            }
        }
    }

    impl Default for Crf {
        fn default() -> Self {
            Self { crf: 50 }
        }
    }

    /// The trade-off between encoding speed and compression efficiency is managed with the -preset option.
    /// Since SVT-AV1 0.9.0, supported presets range from 0 to 13, with higher numbers providing a higher encoding speed.
    /// Note that preset 13 is only meant for debugging and running fast convex-hull encoding.
    /// In versions prior to 0.9.0, valid presets are 0 to 8.
    #[derive(Debug, Clone, Copy)]
    pub struct Preset {
        preset: i32,
    }

    impl TryFrom<i32> for Preset {
        type Error = Report;
        fn try_from(value: i32) -> Result<Self, Self::Error> {
            match value {
                0..=13 => Ok(Preset { preset: value }),
                _ => Err(eyre!("invalid SVT-AV1 preset {}", value)),
            }
        }
    }

    impl Preset {
        pub fn preset(&self) -> i32 {
            self.preset
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct FastDecode {
        fast_decode: i32,
    }

    impl TryFrom<i32> for FastDecode {
        type Error = Report;
        fn try_from(value: i32) -> Result<Self, Self::Error> {
            match value {
                1..=3 => Ok(FastDecode { fast_decode: value }),
                _ => Err(eyre!("invalid SVT-AV1 fast_decode value {}", value)),
            }
        }
    }

    impl FastDecode {
        pub fn fast_decode(&self) -> i32 {
            self.fast_decode
        }
    }

    /// for svt-av1
    #[derive(Debug, Clone)]
    pub struct AV1Target {
        pub crf: Crf,
        pub fast_decode: Option<FastDecode>,
        pub preset: Option<Preset>,
        pub max_bitrate: Option<u32>,
    }
}

#[derive(Debug, Clone)]
pub enum CodecTarget {
    AVC(avc::AVCTarget),
    AV1(av1::AV1Target),
}

#[derive(Debug, Clone, Copy)]
pub enum Scale {
    HeightKeepAspect { height: u32 },
    WidthKeepAspect { width: u32 },
}

#[derive(Debug, Clone)]
pub struct EncodingTarget {
    pub codec: CodecTarget,
    pub scale: Option<Scale>,
}

fn ffmpeg_flags(encoding_target: &EncodingTarget) -> Vec<String> {
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
                f.push(format!("-tune"));
                f.push(tune.to_string());
            }
            if let Some(max_bitrate) = target.max_bitrate {
                f.push(format!("-maxrate"));
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
                f.push(format!("-preset"));
                f.push(preset.preset().to_string());
            }
            if let Some(max_bitrate) = target.max_bitrate {
                f.push(format!("-maxrate"));
                f.push(max_bitrate.to_string());
            }
            if let Some(fast_decode) = target.fast_decode {
                f.push(format!("-svtav1-params"));
                f.push(format!(
                    "fast-decode={}",
                    fast_decode.fast_decode().to_string()
                ));
            }
            f
        }
    };
    if let Some(scale) = encoding_target.scale {
        let scale_multiple: i32 = match encoding_target.codec {
            CodecTarget::AVC(_) => 2,
            CodecTarget::AV1(_) => 2,
        };
        flags.push(format!("-vf"));
        let scale_str = match scale {
            Scale::HeightKeepAspect { height } => format!("-{}:{}", scale_multiple, height),
            Scale::WidthKeepAspect { width } => format!("{}:-{}", width, scale_multiple),
        };
        flags.push(format!("scale={}", scale_str));
    }
    flags
}

pub fn ffmpeg_command(input: &Path, output: &Path, target: EncodingTarget) -> Command {
    let mut command = Command::new("ffmpeg");
    command.arg("-i").arg(input);
    let flags = ffmpeg_flags(&target);
    command.args(flags.as_slice());
    command.arg(output);
    command
}

#[test]
fn ffmpeg_avc_command_assembled_correctly() {
    use avc::*;
    let input = PathBuf::from("/path/to/input.mp4");
    let output = PathBuf::from("out.mp4");
    let codec = CodecTarget::AVC(AVCTarget {
        preset: Preset::Medium,
        tune: Some(Tune::Zerolatency),
        crf: Crf::try_from(24).unwrap(),
        max_bitrate: Some(10_000_000),
    });
    let scale = Some(Scale::WidthKeepAspect { width: 1280 });
    let command = ffmpeg_command(&input, &output, EncodingTarget { codec, scale });
    let expected = "ffmpeg -i /path/to/input.mp4 -c:v libx264 -crf 24 -preset medium -tune zerolatency -maxrate 10000000 -vf scale=1280:-2 out.mp4";
    let actual = format!("{:?}", command.as_std()).replace("\"", "");
    assert_eq!(expected, actual);
}

#[test]
fn ffmpeg_av1_command_assembled_correctly() {
    use av1::*;
    let input = PathBuf::from("/path/to/input.mp4");
    let output = PathBuf::from("out.mp4");
    let codec = CodecTarget::AV1(AV1Target {
        preset: Some(Preset::try_from(8).unwrap()),
        crf: Crf::try_from(45).unwrap(),
        max_bitrate: Some(4_000_000),
        fast_decode: Some(FastDecode::try_from(1).unwrap()),
    });
    let scale: Option<Scale> = Some(Scale::HeightKeepAspect { height: 500 });
    let command = ffmpeg_command(&input, &output, EncodingTarget { codec, scale });
    let expected = "ffmpeg -i /path/to/input.mp4 -c:v libsvtav1 -crf 45 -preset 8 -maxrate 4000000 -svtav1-params fast-decode=1 -vf scale=-2:500 out.mp4";
    let actual = format!("{:?}", command.as_std()).replace("\"", "");
    assert_eq!(expected, actual);
}
