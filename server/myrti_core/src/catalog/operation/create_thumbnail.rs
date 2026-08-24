use camino::Utf8PathBuf as PathBuf;
use eyre::{Context, Report, Result};

use crate::{
    catalog::image_conversion_target::{ImageFormatTarget, heif::AvifTarget},
    core::storage::{Storage, StorageProvider},
    model::{AssetFile, AssetType, FileId, Size, ThumbnailFormat, ThumbnailType},
    processing::{
        self,
        image::thumbnail::{
            ThumbnailParams, ThumbnailResult, generate_thumbnail, generate_video_thumbnail,
        },
        process_control::ProcessControlReceiver,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetThumbnail {
    pub file_id: FileId,
    pub thumbnails: Vec<ThumbnailToCreate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAssetThumbhash {
    pub file_id: FileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreate {
    pub ty: ThumbnailType,
    pub formats: Vec<ThumbnailFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateThumbnailWithPaths {
    pub file_id: FileId,
    pub thumbnails: Vec<ThumbnailToCreateWithPaths>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailToCreateWithPaths {
    pub ty: ThumbnailType,
    pub file_keys: Vec<(ThumbnailFormat, String)>,
}

#[derive(Debug, Clone)]
pub struct ThumbnailSideEffectSuccess {
    pub ty: ThumbnailType,
    pub format: ThumbnailFormat,
    pub actual_size: Size,
}

#[derive(Debug)]
pub struct ThumbnailSideEffectResult {
    pub file_id: FileId,
    pub succeeded: Vec<ThumbnailSideEffectSuccess>,
    pub failed: Vec<(ThumbnailToCreateWithPaths, Report)>,
}

#[tracing::instrument(skip_all, fields(path=?asset_path, ?thumb), level = "trace", err)]
pub async fn create_thumbnail(
    asset_path: PathBuf,
    file: &AssetFile,
    thumb: &ThumbnailToCreateWithPaths,
    storage: &Storage,
    control_recv: &mut ProcessControlReceiver,
) -> Result<ThumbnailResult> {
    let out_dimension = match thumb.ty {
        ThumbnailType::SmallSquare => processing::image::OutDimension::Crop {
            width: 200,
            height: 200,
        },
        ThumbnailType::LargeOrigAspect => processing::image::OutDimension::KeepAspect {
            width: (file.size.width as f32 * (300.0 / file.size.height as f32)).round() as i32,
        },
    };
    let mut outputs = Vec::new();
    for (format, file_key) in &thumb.file_keys {
        let target = match format {
            ThumbnailFormat::Webp => ImageFormatTarget::WEBP,
            ThumbnailFormat::Avif => ImageFormatTarget::AVIF(AvifTarget {
                quality: 50.try_into()?,
                effort: 7.try_into()?,
                ..Default::default()
            }),
        };
        outputs.push((storage.local_path(file_key).await?.unwrap(), target));
    }
    let thumbnail_params = ThumbnailParams {
        in_path: asset_path,
        outputs,
        out_dimension,
    };
    match file.ty {
        AssetType::Image => generate_thumbnail(thumbnail_params).await,
        AssetType::Video => generate_video_thumbnail(thumbnail_params, control_recv).await,
    }
}
