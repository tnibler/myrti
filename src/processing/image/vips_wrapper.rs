use eyre::{eyre, Context, Result};
use std::{
    ffi::{c_char, CString},
    os::unix::prelude::OsStrExt,
    path::PathBuf,
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
        wrapper::teardown();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailParams {
    pub in_path: PathBuf,
    pub out_paths: Vec<PathBuf>,
    pub width: i32,
    pub height: i32,
}

pub fn generate_thumbnail(params: ThumbnailParams) -> Result<()> {
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
        width: params.width,
        height: params.height,
    };
    let ret = unsafe { wrapper::thumbnail(params) };
    if ret != 0 {
        return Err(eyre!(
            "An error occurred while creating thumbnail with libvips"
        ));
    }
    Ok(())
}
