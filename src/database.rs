use std::fmt;

use diesel::connection::SimpleConnection;
use diesel::r2d2;
use diesel::r2d2::CustomizeConnection;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use lazy_static::lazy_static;

use crate::config;

type ManageConnectionType = r2d2::ConnectionManager<SqliteConnection>;
pub type Pool = r2d2::Pool<ManageConnectionType>;
pub type PooledConnection = diesel::r2d2::PooledConnection<ManageConnectionType>;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

lazy_static! {
    pub static ref POOL: Pool = PoolBuilder::new().build();
}

#[derive(Copy, Clone, Debug)]
struct ConnectionCustomizer {}

impl<C, E> CustomizeConnection<C, E> for ConnectionCustomizer
where
    C: SimpleConnection,
{
    fn on_acquire(&self, conn: &mut C) -> std::result::Result<(), E> {
        log::debug!("enabling foreign keys on connection");
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("unable to enable foreign keys");

        Ok(())
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

        let customizer = Box::new(ConnectionCustomizer {});

        let pool = Pool::builder()
            .max_size(self.cfg.pool_size)
            .connection_customizer(customizer)
            .build(r2d2::ConnectionManager::new(&self.cfg.url))
            .expect("Failed to create database connection pool");

        let mut conn = pool.get().expect("unable to get connection");
        run_migrations(&mut conn).expect("unable to run migrations");
        pool
    }
}

pub enum Error {
    MigrationError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::MigrationError => write!(f, "Migration error"),
        }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::MigrationError => write!(f, "Migration error"),
        }
    }
}

impl std::error::Error for Error {}

pub fn run_migrations(conn: &mut PooledConnection) -> std::result::Result<(), Error> {
    log::info!("running migrations");
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(_) => Ok(()),
        _ => Err(Error::MigrationError),
    }
}
