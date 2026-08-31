pub mod actor;
pub mod catalog;
pub mod config;
pub mod core;
pub mod processing;
pub use processing::startup_self_check;
pub mod util;

pub fn global_init() {
    processing::image::vips_init();
}
