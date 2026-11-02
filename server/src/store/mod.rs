//! The relational store: the control-plane data, in SQLite, Postgres or
//! MariaDB. The rules every table and query follow are in this directory's
//! `AGENTS.md`.

pub mod migrate;

use std::str::FromStr;

use sqlx::migrate::MigrateDatabase;
use sqlx::mysql::{MySql, MySqlPool};
use sqlx::postgres::{PgPool, Postgres};
use sqlx::sqlite::{Sqlite, SqliteConnectOptions, SqlitePool};
use sqlx::{Database, Executor};

/// An open store on one of the three backends. Clones share one connection
/// pool.
#[derive(Clone)]
pub enum Store {
    Sqlite(SqlitePool),
    Postgres(PgPool),
    MariaDb(MySqlPool),
}

/// The database a URL names.
enum Backend {
    Sqlite,
    Postgres,
    MariaDb,
}

/// The backend `url`'s scheme names.
///
/// # Errors
///
/// Fails on any other scheme, and the message lists the accepted ones.
fn backend(url: &str) -> Result<Backend, sqlx::Error> {
    let scheme = url.split_once(':').map_or("", |(scheme, _)| scheme);
    // Each driver's own scheme list, so the accepted schemes are the ones the
    // driver parses.
    let backends = [
        (Sqlite::URL_SCHEMES, Backend::Sqlite),
        (Postgres::URL_SCHEMES, Backend::Postgres),
        (MySql::URL_SCHEMES, Backend::MariaDb),
    ];
    let accepted: Vec<&str> = backends
        .iter()
        .flat_map(|(s, _)| s.iter().copied())
        .collect();
    backends
        .into_iter()
        .find(|(schemes, _)| schemes.contains(&scheme))
        .map(|(_, backend)| backend)
        .ok_or_else(|| {
            sqlx::Error::Configuration(
                format!(
                    "the database URL scheme {scheme:?} is not one of {}",
                    accepted.join(", ")
                )
                .into(),
            )
        })
}

/// Opens the store at `url` for serve.
///
/// # Errors
///
/// Fails when the URL does not parse, or when the database does not exist or
/// cannot be opened.
pub async fn open(url: &str) -> Result<Store, sqlx::Error> {
    match backend(url)? {
        Backend::Sqlite => open_sqlite(url, false).await,
        Backend::Postgres => Ok(Store::Postgres(PgPool::connect(url).await?)),
        Backend::MariaDb => Ok(Store::MariaDb(MySqlPool::connect(url).await?)),
    }
}

/// Opens the store at `url` for migrate, creating the database when it is
/// missing.
///
/// # Errors
///
/// Fails when the URL does not parse, or when the database cannot be created
/// or opened.
pub async fn create(url: &str) -> Result<Store, sqlx::Error> {
    match backend(url)? {
        // Creation is turned on here instead of with `mode=rwc` in the URL, so
        // migrate and serve take the same URL.
        Backend::Sqlite => open_sqlite(url, true).await,
        // The servers create a database with a statement, which needs a user
        // allowed to create databases.
        Backend::Postgres => {
            create_missing::<Postgres>(url).await?;
            open(url).await
        }
        Backend::MariaDb => {
            create_missing::<MySql>(url).await?;
            open(url).await
        }
    }
}

/// Whether the store answers a trivial query.
pub async fn ping(store: &Store) -> bool {
    match store {
        Store::Sqlite(pool) => pool.execute("SELECT 1").await.is_ok(),
        Store::Postgres(pool) => pool.execute("SELECT 1").await.is_ok(),
        Store::MariaDb(pool) => pool.execute("SELECT 1").await.is_ok(),
    }
}

async fn open_sqlite(url: &str, create: bool) -> Result<Store, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?.create_if_missing(create);
    Ok(Store::Sqlite(SqlitePool::connect_with(options).await?))
}

/// Creates the database `url` names on its server, unless it exists.
async fn create_missing<D: MigrateDatabase>(url: &str) -> Result<(), sqlx::Error> {
    if !D::database_exists(url).await? {
        D::create_database(url).await?;
    }
    Ok(())
}
