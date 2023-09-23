use super::operation::package_video::AudioEncodingTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoEncodingTarget {
    pub codec: CodecTarget,
    pub scale: Option<Scale>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecTarget {
    AVC(avc::AVCTarget),
    AV1(av1::AV1Target),
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scale {
    HeightKeepAspect { height: u32 },
    WidthKeepAspect { width: u32 },
}

pub mod avc {
    use std::fmt::Display;

    use eyre::{eyre, Report};

    #[allow(dead_code)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AVCTarget {
        pub preset: Preset,
        pub tune: Option<Tune>,
        pub crf: Crf,
        pub max_bitrate: Option<u32>,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Preset {
        Ultrafast,
        Superfast,
        Veryfast,
        Faster,
        Fast,
        Medium,
        Slow,
        Slower,
        Veryslow,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Tune {
        Film,
        Animation,
        Grain,
        Stillimage,
        Fastdecode,
        Zerolatency,
    }

    /// https://trac.ffmpeg.org/wiki/Encode/H.264
    /// The range of the CRF scale is 0–51, where 0 is lossless (for 8 bit only, for 10 bit use -qp 0), 23 is the default, and 51 is worst quality possible.
    /// A lower value generally leads to higher quality, and a subjectively sane range is 17–28.
    /// Consider 17 or 18 to be visually lossless or nearly so; it should look the same or nearly the same as the input but it isn't technically lossless.
    /// The range is exponential, so increasing the CRF value +6 results in roughly half the bitrate / file size, while -6 leads to roughly twice the bitrate.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Crf {
        crf: i32,
    }

    impl Default for Preset {
        fn default() -> Self {
            Self::Medium
        }
    }

    impl Display for Preset {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}",
                match self {
                    Self::Ultrafast => "ultrafast",
                    Self::Superfast => "superfast",
                    Self::Veryfast => "veryfast",
                    Self::Faster => "faster",
                    Self::Fast => "fast",
                    Self::Medium => "medium",
                    Self::Slow => "slow",
                    Self::Slower => "slower",
                    Self::Veryslow => "veryslow",
                }
            )
        }
    }

    impl Display for Tune {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}",
                match self {
                    Self::Film => "film",
                    Self::Animation => "animation",
                    Self::Grain => "grain",
                    Self::Stillimage => "stillimage",
                    Self::Fastdecode => "fastdecode",
                    Self::Zerolatency => "zerolatency",
                }
            )
        }
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
}

pub mod av1 {
    use eyre::{eyre, Report};

    /// For libsvtav1 only
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AV1Target {
        pub crf: Crf,
        pub fast_decode: Option<FastDecode>,
        pub preset: Option<Preset>,
        pub max_bitrate: Option<u32>,
    }

    /// https://trac.ffmpeg.org/wiki/Encode/AV1#CRF
    /// The valid CRF value range is 0-63, with the default being 50.
    /// Lower values correspond to higher quality and greater file size.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Crf {
        crf: i32,
    }

    /// The trade-off between encoding speed and compression efficiency is managed with the -preset option.
    /// Since SVT-AV1 0.9.0, supported presets range from 0 to 13, with higher numbers providing a higher encoding speed.
    /// Note that preset 13 is only meant for debugging and running fast convex-hull encoding.
    /// In versions prior to 0.9.0, valid presets are 0 to 8.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Preset {
        preset: i32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FastDecode {
        fast_decode: i32,
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
}

/// name used by ffmpeg
pub fn codec_name(target: &CodecTarget) -> String {
    match target {
        CodecTarget::AVC(_) => "h264",
        CodecTarget::AV1(_) => "av1",
    }
    .to_string()
}

pub fn audio_codec_name(target: &AudioEncodingTarget) -> String {
    match target {
        AudioEncodingTarget::AAC => "aac",
        AudioEncodingTarget::OPUS => "opus",
        AudioEncodingTarget::FLAC => "flac",
        AudioEncodingTarget::MP3 => "mp3",
    }
    .into()
}
