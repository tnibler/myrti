use std::{
    ffi::{c_char, CString},
    os::unix::prelude::OsStrExt,
    sync::Once,
};

use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use eyre::{eyre, Context, Result};
use tracing::{error, info_span};

use crate::catalog::image_conversion_target::{
    heif::{BitDepth, Compression},
    ImageConversionTarget, ImageFormatTarget,
};

#[allow(non_snake_case)]
mod wrapper {
    include!(concat!(env!("OUT_DIR"), "/vips_wrapper_bindings.rs"));
}

static VIPS_INITIALIZED: Once = Once::new();

pub fn init() {
    VIPS_INITIALIZED.call_once(|| unsafe {
        let span = info_span!("libvips initialization");
        let _enter = span.enter();
        let ret = wrapper::init();
        if ret != 0 {
            error!("Could not initialize libvips");
        }
    })
}

// Not sure when we would actually call this,
// maybe reinitialize everytime we use vips?
pub fn teardown() {
    unsafe {
        let span = info_span!("libvips teardown");
        let _enter = span.enter();
        wrapper::teardown();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutDimension {
    KeepAspect { width: i32 },
    Crop { width: i32, height: i32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VipsThumbnailParams {
    pub in_path: PathBuf,
    pub out_paths: Vec<PathBuf>,
    pub out_dimension: OutDimension,
}

#[tracing::instrument(level = "debug")]
pub fn generate_thumbnail(params: VipsThumbnailParams) -> Result<()> {
    // let span = debug_span!("Generate image thumbnail (libvips)");
    // let _enter = span.enter();
    let c_path = CString::new(params.in_path.as_os_str().as_bytes()).wrap_err(format!(
        "Could not convert path {} to bytes",
        &params.in_path
    ))?;
    // c_out_paths has to stay alive for as long as c_out_path_ptrs is used
    let c_out_paths = params
        .out_paths
        .into_iter()
        .map(|path| {
            CString::new(path.as_os_str().as_bytes())
                .wrap_err(format!("Could not convert path {} to bytes", &path))
        })
        .collect::<Result<Vec<_>>>()?;
    let c_out_path_ptrs: Vec<*const c_char> =
        c_out_paths.iter().map(|c_str| c_str.as_ptr()).collect();
    let params = wrapper::ThumbnailParams {
        in_path: c_path.as_ptr(),
        out_paths: c_out_path_ptrs.as_ptr(),
        num_out_paths: c_out_path_ptrs.len() as u64,
        width: match params.out_dimension {
            OutDimension::KeepAspect { width } => width,
            OutDimension::Crop { width, height: _ } => width,
        },
        height: match params.out_dimension {
            OutDimension::KeepAspect { width: _ } => 0,
            OutDimension::Crop { width: _, height } => height,
        },
        keep_aspect: match params.out_dimension {
            OutDimension::KeepAspect { width: _ } => true,
            OutDimension::Crop {
                width: _,
                height: _,
            } => false,
        },
    };
    let ret = unsafe { wrapper::thumbnail(params) };
    if ret != 0 {
        return Err(eyre!(
            "An error occurred while creating thumbnail with libvips"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

pub fn get_image_size(path: &Path) -> Result<Size> {
    let c_path = CString::new(path.as_os_str().as_bytes())
        .wrap_err(format!("Could not convert path {} to bytes", &path))?;
    let mut out = wrapper::ImageInfo {
        width: 0,
        height: 0,
    };
    let ret = unsafe { wrapper::read_image_info(c_path.as_ptr(), &mut out as *mut _) };
    match ret {
        0 => Ok(Size {
            width: out.width,
            height: out.height,
        }),
        _ => Err(eyre!("Error getting image info with libvips")),
    }
}

#[tracing::instrument(level = "debug")]
pub fn convert_image(
    input: &Path,
    output: &Path,
    target: &ImageConversionTarget,
) -> Result<Option<Size>> {
    let c_in_path = CString::new(input.as_os_str().as_bytes())
        .wrap_err(format!("Could not convert path {} to bytes", input))?;
    let c_out_path = CString::new(output.as_os_str().as_bytes())
        .wrap_err(format!("Could not convert path {} to bytes", output))?;
    let c_scale = wrapper::Scale {
        do_scale: target.scale.is_some(),
        scale: target.scale.unwrap_or(0.0),
    };
    match &target.format {
        ImageFormatTarget::AVIF(avif) => {
            let c_save_params = wrapper::HeifSaveParams {
                quality: avif.quality.into(),
                lossless: if avif.lossless { 1 } else { 0 },
                bit_depth: match avif.bit_depth {
                    BitDepth::Eight => 8,
                    BitDepth::Ten => 10,
                    BitDepth::Twelve => 12,
                },
                compression: match avif.compression {
                    Compression::HEVC => 1,
                    Compression::AVC => 2,
                    Compression::JPEG => 3,
                    Compression::AV1 => 4,
                },
            };
            let ret = unsafe {
                wrapper::convert_heif(
                    c_in_path.as_ptr(),
                    c_out_path.as_ptr(),
                    c_save_params,
                    c_scale,
                )
            };
            match ret.err {
                0 => match target.scale {
                    None => Ok(None),
                    Some(_size) => Ok(Some(Size {
                        width: ret.width,
                        height: ret.height,
                    })),
                },
                _ => Err(eyre!("Error converting image to HEIF with libvips")),
            }
        }
        ImageFormatTarget::JPEG(jpeg) => {
            let c_save_params = wrapper::JpegSaveParams {
                quality: jpeg.quality.into(),
            };
            let ret = unsafe {
                wrapper::convert_jpeg(
                    c_in_path.as_ptr(),
                    c_out_path.as_ptr(),
                    c_save_params,
                    c_scale,
                )
            };
            match ret.err {
                0 => match target.scale {
                    None => Ok(None),
                    Some(_size) => Ok(Some(Size {
                        width: ret.width,
                        height: ret.height,
                    })),
                },
                _ => Err(eyre!("Error converting image to HEIF with libvips")),
            }
        }
    }
}
