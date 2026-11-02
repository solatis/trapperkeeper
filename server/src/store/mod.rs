//! The relational store: the control-plane data, in SQLite. The rules every
//! table and query follow are in this directory's `AGENTS.md`.

pub mod migrate;

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

/// An open store. Clones share one connection pool.
#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

/// Opens the store at `url` for migrate, creating the database when it is
/// missing.
///
/// # Errors
///
/// Fails when the URL does not parse or the database cannot be opened.
pub async fn create(url: &str) -> Result<Store, sqlx::Error> {
    // Creation is turned on here instead of with `mode=rwc` in the URL, so
    // migrate and serve take the same URL.
    let options = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    Ok(Store { pool })
}
