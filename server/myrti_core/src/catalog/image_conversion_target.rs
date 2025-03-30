#[derive(Debug, Clone)]
pub struct ImageConversionTarget {
    pub format: ImageFormatTarget,
    pub scale: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageFormatTarget {
    AVIF(heif::AvifTarget),
    JPEG(jpeg::JpegTarget),
}

pub mod jpeg {
    use eyre::eyre;

    #[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
    pub struct JpegTarget {
        pub quality: QualityFactor,
    }

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct QualityFactor(i32);

    impl TryFrom<i32> for QualityFactor {
        type Error = eyre::Report;

        fn try_from(value: i32) -> Result<Self, Self::Error> {
            match value {
                1..=100 => Ok(QualityFactor(value)),
                _ => Err(eyre!("invalid JPEG quality factor {}", value)),
            }
        }
    }

    impl From<QualityFactor> for i32 {
        fn from(val: QualityFactor) -> Self {
            val.0
        }
    }

    impl Default for QualityFactor {
        fn default() -> Self {
            Self(75)
        }
    }
}

pub mod heif {
    use eyre::eyre;

    #[allow(dead_code)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub enum BitDepth {
        Eight,
        Ten,
        Twelve,
    }

    #[allow(dead_code)]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub enum Compression {
        HEVC,
        AVC,
        JPEG,
        AV1,
    }

    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct QualityFactor(i32);

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub struct AvifTarget {
        pub quality: QualityFactor,
        pub lossless: bool,
        pub bit_depth: BitDepth,
        pub compression: Compression,
    }

    impl TryFrom<i32> for QualityFactor {
        type Error = eyre::Report;

        fn try_from(value: i32) -> Result<Self, Self::Error> {
            match value {
                1..=100 => Ok(QualityFactor(value)),
                _ => Err(eyre!("invalid HEIF quality factor {}", value)),
            }
        }
    }

    impl From<QualityFactor> for i32 {
        fn from(val: QualityFactor) -> Self {
            val.0
        }
    }

    impl Default for QualityFactor {
        fn default() -> Self {
            Self(50)
        }
    }

    impl Default for AvifTarget {
        fn default() -> Self {
            Self {
                quality: Default::default(),
                lossless: false,
                bit_depth: BitDepth::Eight,
                compression: Compression::AV1,
            }
        }
    }
}

pub fn image_format_name(format_target: &ImageFormatTarget) -> &'static str {
    match format_target {
        ImageFormatTarget::AVIF(_) => "avif",
        ImageFormatTarget::JPEG(_) => "jpeg",
    }
}
