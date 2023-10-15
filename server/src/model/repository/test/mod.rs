use chrono::SubsecRound;
use sqlx::SqlitePool;

use super::pool::DbPool;

pub mod proptest_arb;
pub mod asset;
pub mod asset_root_dir;
pub mod data_dir;
pub mod image_representation;
pub mod representation;

pub async fn create_db() -> DbPool {
    let db_url = "sqlite::memory:";
    let pool = SqlitePool::connect(db_url).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

pub fn utc_now_millis_zero() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now().trunc_subsecs(3)
}
