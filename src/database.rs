use diesel::connection::SimpleConnection; // for batch_execute
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use dotenvy::dotenv;
use std::env;
use std::fmt;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn establish_connection() -> Result<SqliteConnection, diesel::result::Error> {
    dotenv().ok();

    let url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    establish_connection_from_url(url)
}

pub fn establish_connection_from_url(
    url: String,
) -> Result<SqliteConnection, diesel::result::Error> {
    let mut conn = SqliteConnection::establish(&url)
        .unwrap_or_else(|_| panic!("Unable to open database {}", url));

    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("Unable to enable foreign keys");

    Ok(conn)
}

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

pub fn pool() -> DbPool {
    dotenv().ok();

    let url: String = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let n: Option<u32> = match env::var("DATABASE_POOL_SIZE") {
        Ok(s) => Some(s.parse().unwrap()),
        _ => None,
    };

    pool_from_url(url, n)
}

const DEFAULT_POOL_SIZE: u32 = 8;

pub fn pool_from_url(url: String, size: Option<u32>) -> DbPool {
    let n = size.unwrap_or(DEFAULT_POOL_SIZE);

    log::info!("building pool with size {} from url {}", n, url);
    DbPool::builder()
        .max_size(n)
        .build(ConnectionManager::new(url))
        .expect("Failed to create database connection pool")
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
    match conn.run_pending_migrations(MIGRATIONS) {
        Ok(_) => Ok(()),
        _ => Err(Error::MigrationError),
    }
}
