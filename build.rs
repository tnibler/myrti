use std::{env, path::PathBuf, process::Command};

fn main() {
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=vips_wrapper");

    pkg_config::Config::new()
        .probe("vips")
        .expect("libvips not found");
    pkg_config::Config::new()
        .probe("glib-2.0")
        .expect("glib-2.0 not found");
    pkg_config::Config::new()
        .probe("gobject-2.0")
        .expect("gobject-2.0 not found");

    let pkg_config_out = Command::new("sh")
        .arg("-c")
        .arg("pkg-config --cflags --libs vips glib-2.0 gobject-2.0")
        .output()
        .unwrap()
        .stdout;

    let out_str = String::from_utf8_lossy(&pkg_config_out);
    let flags: Vec<&str> = out_str.split(' ').map(|part| part.trim()).collect();

    let mut wrapper_cc = cc::Build::new();
    wrapper_cc.file("vips_wrapper/thumbnail.c");
    for flag in flags {
        if !flag.is_empty() {
            wrapper_cc.flag(flag);
        }
    }
    wrapper_cc.compile("vips_wrapper");

    let bindings = bindgen::Builder::default()
        .header("vips_wrapper/vips_wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Failed to generate vips_wrapper bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("vips_wrapper_bindings.rs"))
        .expect("Couldn't write bindings");
    println!("cargo:rustc-link-lib=glib-2.0");
    println!("cargo:rustc-link-lib=gobject-2.0");
    println!("cargo:rustc-link-lib=vips");
}
