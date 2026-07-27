mod ffmpeg_snapshot;
mod vips_wrapper;

pub use vips_wrapper::OutDimension;
pub use vips_wrapper::init as vips_init;
pub use vips_wrapper::teardown as vips_teardown;
pub use vips_wrapper::{
    VipsThumbnailParams, convert_image, generate_thumbnail, get_image_size, save_test_heif_image,
    save_test_jpeg_image, save_test_webp_image,
};

pub mod image_conversion;
pub mod thumbnail;
