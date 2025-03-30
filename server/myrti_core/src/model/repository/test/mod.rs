use chrono::SubsecRound;

use super::db;

pub mod album;
pub mod asset;
pub mod asset_root_dir;
pub mod image_representation;
pub mod proptest_arb;
pub mod representation;
pub mod timeline;
pub mod timeline_group;
pub mod util;

pub fn utc_now_millis_zero() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now().trunc_subsecs(3)
}
