use sqlx::SqlitePool;

use super::pool::DbPool;

pub mod asset;
pub mod asset_root_dir;
pub mod data_dir;
pub mod representation;

pub async fn create_db() -> DbPool {
    let db_url = "sqlite::memory:";
    let pool = SqlitePool::connect(db_url).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}
