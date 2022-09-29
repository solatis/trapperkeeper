use derive_more;
use diesel;
use diesel::connection::SimpleConnection;
use diesel::r2d2::CustomizeConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use lazy_static::lazy_static;
use r2d2;
use std::time::Duration;

use crate::config;

#[derive(Debug, derive_more::Display, derive_more::Error, derive_more::From)]
pub enum Error {
    #[from]
    DbError(r2d2::Error),

    MigrationError,
}

type InnerConnection = SqliteConnection;
type InnerConnectionManager = diesel::r2d2::ConnectionManager<InnerConnection>;
type InnerPooledConnection = diesel::r2d2::PooledConnection<InnerConnectionManager>;
type InnerPool = diesel::r2d2::Pool<InnerConnectionManager>;

// pub type PooledConnection = diesel::r2d2::PooledConnection<InnerConnectionType>;

pub type Connection = InnerConnection;

#[derive(derive_more::AsMut, derive_more::AsRef)]
pub struct PooledConnection(InnerPooledConnection);

pub struct Pool(InnerPool);

impl Pool {
    pub fn get(&self) -> Result<PooledConnection, Error> {
        let p: &InnerPool = &self.0;
        match p.get() {
            Ok(conn) => {
                log::info!("acquired connection");
                Ok(PooledConnection(conn))
            }
            Err(e) => {
                log::error!("Unable to acquire connection from pool: {}", e);
                Err(e.into())
            }
        }
    }
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

lazy_static! {
    pub static ref POOL: Pool = PoolBuilder::new().build();
}

#[derive(Copy, Clone, Debug)]
struct ConnectionOptions {
    pub enable_wal: bool,
    pub enable_fkey: bool,
    pub busy_timeout: Option<Duration>,
}

impl CustomizeConnection<InnerConnection, diesel::r2d2::Error> for ConnectionOptions {
    fn on_acquire(
        &self,
        conn: &mut InnerConnection,
    ) -> std::result::Result<(), diesel::r2d2::Error> {
        (|| {
            if self.enable_wal {
                log::debug!("enabling WAL on connection");
                conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
            }

            if self.enable_fkey {
                log::debug!("enabling foreign keys on connection");
                conn.batch_execute("PRAGMA foreign_keys = ON")?;
            }

            if let Some(d) = self.busy_timeout {
                log::debug!("setting busy timeout to {}ms", d.as_millis());
                conn.batch_execute(&format!("PRAGMA busy_timeout = {};", d.as_millis()))?;
            }
            Ok(())
        })()
        .map_err(diesel::r2d2::Error::QueryError)
    }
}

pub struct PoolBuilder {
    cfg: config::Database,
}

impl PoolBuilder {
    pub fn new() -> Self {
        PoolBuilder {
            cfg: config::CONFIG.database.clone(),
        }
    }

    pub fn build(&mut self) -> Pool {
        log::info!(
            "building connection pool from url {} with pool size {}",
            self.cfg.url,
            self.cfg.pool_size
        );

        let customizer = Box::new(ConnectionOptions {
            enable_wal: true,
            enable_fkey: true,
            busy_timeout: Some(Duration::from_secs(30)),
        });

        let pool = InnerPool::builder()
            .max_size(self.cfg.pool_size)
            .connection_customizer(customizer)
            .build(diesel::r2d2::ConnectionManager::new(&self.cfg.url))
            .expect("Failed to create database connection pool");

        let mut conn = pool.get().expect("unable to get connection");
        run_migrations(&mut conn).expect("unable to run migrations");
        Pool(pool)
    }
}

pub fn run_migrations(conn: &mut Connection) -> std::result::Result<(), Error> {
    log::info!("running migrations");
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(_) => Ok(()),
        _ => Err(Error::MigrationError),
    }
}
