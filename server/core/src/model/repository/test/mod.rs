use std::str::FromStr;

use chrono::SubsecRound;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};

use super::pool::DbPool;

pub mod album;
pub mod asset;
pub mod asset_root_dir;
pub mod data_dir;
pub mod image_representation;
pub mod proptest_arb;
pub mod representation;
pub mod util;

pub async fn create_db() -> DbPool {
    let connect_options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
    let pool = SqlitePool::connect_with(connect_options).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

pub fn utc_now_millis_zero() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now().trunc_subsecs(3)
}
