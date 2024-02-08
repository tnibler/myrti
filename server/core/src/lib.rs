pub mod catalog;
pub mod config;
pub mod core;
pub mod job;
pub mod model;
mod processing;
pub use deadpool_diesel;

pub fn global_init() {
    processing::image::vips_init();
}
