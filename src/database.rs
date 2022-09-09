use diesel::connection::Connection;
use diesel::connection::SimpleConnection;
use diesel::r2d2;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::fmt;

use crate::config;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

type ManageConnectionType = r2d2::ConnectionManager<SqliteConnection>;
pub type Pool = r2d2::Pool<ManageConnectionType>;

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

        Pool::builder()
            .max_size(self.cfg.pool_size)
            .build(r2d2::ConnectionManager::new(&self.cfg.url))
            .expect("Failed to create database connection pool")
    }
}

pub struct ConnectionBuilder {
    cfg: config::Database,
}

impl ConnectionBuilder {
    pub fn new() -> Self {
        ConnectionBuilder {
            cfg: config::CONFIG.database.clone(),
        }
    }

    pub fn build(&mut self) -> SqliteConnection {
        let mut conn = SqliteConnection::establish(&self.cfg.url)
            .expect("unable to establish sqlite connection");

        conn.batch_execute("PRAGMA foreign_keys = ON")
            .expect("unable to set pragma");

        conn
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

pub fn run_migrations(conn: &mut SqliteConnection) -> std::result::Result<(), Error> {
    log::info!("running migrations");
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(_) => Ok(()),
        _ => Err(Error::MigrationError),
    }
}
