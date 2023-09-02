use eyre::{eyre, Context, Result};
use std::{
    ffi::{c_char, CString},
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    sync::Once,
};
use tracing::{error, info_span};

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

pub fn generate_thumbnail(params: VipsThumbnailParams) -> Result<()> {
    let span = info_span!("Generate image thumbnail (libvips)");
    let _enter = span.enter();
    let c_path = CString::new(params.in_path.as_os_str().as_bytes()).wrap_err(format!(
        "Could not convert path {} to bytes",
        params.in_path.display()
    ))?;
    // c_out_paths has to stay alive for as long as c_out_path_ptrs is used
    let c_out_paths = params
        .out_paths
        .into_iter()
        .map(|path| {
            CString::new(path.as_os_str().as_bytes()).wrap_err(format!(
                "Could not convert path {} to bytes",
                path.display()
            ))
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
            OutDimension::KeepAspect { width: _ } => 1,
            OutDimension::Crop {
                width: _,
                height: _,
            } => 0,
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
    let c_path = CString::new(path.as_os_str().as_bytes()).wrap_err(format!(
        "Could not convert path {} to bytes",
        path.display()
    ))?;
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
