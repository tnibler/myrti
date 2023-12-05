pub mod catalog;
pub mod config;
pub mod core;
pub mod job;
pub mod model;
mod processing;

pub fn global_init() {
    processing::image::vips_init();
}
