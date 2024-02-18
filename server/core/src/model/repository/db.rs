use deadpool_diesel::sqlite::{Hook, Manager, Object};
use deadpool_diesel::Pool;
use diesel::connection::SimpleConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use eyre::{Context, Result};

pub(super) const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn open_db_pool(sqlite_url: &str) -> Result<DbPool> {
    let manager = Manager::new(sqlite_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager)
        .max_size(8)
        .post_create(Hook::sync_fn(|conn, _| {
            let mut conn = conn.lock().unwrap();
            let res = connection_setup(&mut conn);
            match res {
                Ok(_) => Ok(()),
                // hopeless wrangling with error types here, idk how to get the diesel error
                // into a HookError
                Err(_err) => Err(deadpool::managed::HookError::StaticMessage(
                    "error configuring database connection",
                )),
            }
        }))
        .build()
        .wrap_err("error creating database pool")?;
    Ok(DbPool::new(pool))
}

#[cfg(test)]
pub fn open_in_memory_and_migrate() -> diesel::sqlite::SqliteConnection {
    use diesel::Connection;
    let mut conn = diesel::sqlite::SqliteConnection::establish(":memory:")
        .expect("error opening in memory db");
    connection_setup(&mut conn).expect("error configuring in memory db connection");
    migrate(&mut conn).expect("error running migrations on in memory connection");
    conn
}

pub fn migrate(conn: &mut diesel::SqliteConnection) -> Result<()> {
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(_) => {}
        Err(e) => return Err(eyre::eyre!("error running migrations")),
    }
    Ok(())
}

fn connection_setup(conn: &mut diesel::SqliteConnection) -> Result<()> {
    conn.batch_execute(
        r#"
PRAGMA journal_mode = wal;
PRAGMA foreign_keys = on;
    "#,
    )?;
    Ok(())
}

type SqlitePool = Pool<Manager>;

pub type PooledDbConn = deadpool_diesel::Connection<diesel::SqliteConnection>;
pub type DbConn = diesel::SqliteConnection;

#[derive(Clone)]
pub struct DbPool {
    pool: SqlitePool,
}

impl DbPool {
    pub(self) fn new(pool: SqlitePool) -> Self {
        DbPool { pool }
    }

    pub async fn get(&self) -> Result<Object> {
        self.pool
            .get()
            .await
            .wrap_err("could not acquire db connection")
    }
}
