//! The relational store: the control-plane data, in SQLite, Postgres or
//! MariaDB. The rules every table and query follow are in this directory's
//! `AGENTS.md`. Store functions take a `Tx` and never commit, so a caller
//! composes them in one transaction.

pub mod installation;
pub mod migrate;
pub mod users;

use std::borrow::Cow;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::migrate::MigrateDatabase;
use sqlx::mysql::{MySql, MySqlPool};
use sqlx::postgres::{PgPool, Postgres};
use sqlx::sqlite::{Sqlite, SqliteConnectOptions, SqlitePool};
use sqlx::{Database, Executor, Transaction};

/// The id of the default tenant, which the first migration seeds.
pub const DEFAULT_TENANT_ID: &str = "00000000-0000-7000-8000-000000000000";

/// An open store on one of the three backends. Clones share one connection
/// pool.
#[derive(Clone)]
pub enum Store {
    Sqlite(SqlitePool),
    Postgres(PgPool),
    MariaDb(MySqlPool),
}

/// An open transaction on one of the three backends. Store functions take it,
/// and the caller commits it or drops it to roll back.
pub enum Tx {
    Sqlite(Transaction<'static, Sqlite>),
    Postgres(Transaction<'static, Postgres>),
    MariaDb(Transaction<'static, MySql>),
}

impl Store {
    /// Opens a transaction.
    ///
    /// # Errors
    ///
    /// Fails when no connection can be acquired or the transaction cannot
    /// begin.
    pub async fn begin(&self) -> Result<Tx, sqlx::Error> {
        Ok(match self {
            Self::Sqlite(pool) => Tx::Sqlite(pool.begin().await?),
            Self::Postgres(pool) => Tx::Postgres(pool.begin().await?),
            Self::MariaDb(pool) => Tx::MariaDb(pool.begin().await?),
        })
    }
}

impl Tx {
    /// Commits the transaction.
    ///
    /// # Errors
    ///
    /// Fails when the database refuses the commit.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        match self {
            Self::Sqlite(tx) => tx.commit().await,
            Self::Postgres(tx) => tx.commit().await,
            Self::MariaDb(tx) => tx.commit().await,
        }
    }
}

/// `statement` with its placeholders as the backend of `tx` spells them:
/// MariaDB takes `?` in place of `$1`, `$2` and so on.
fn sql(tx: &Tx, statement: &'static str) -> Cow<'static, str> {
    match tx {
        Tx::Sqlite(_) | Tx::Postgres(_) => Cow::Borrowed(statement),
        // A statement uses each number once, in ascending order, because `?`
        // binds by position (doc/adr/0019).
        Tx::MariaDb(_) => {
            let mut out = String::with_capacity(statement.len());
            let mut chars = statement.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '$' && chars.peek().is_some_and(char::is_ascii_digit) {
                    out.push('?');
                    while chars.peek().is_some_and(char::is_ascii_digit) {
                        chars.next();
                    }
                } else {
                    out.push(c);
                }
            }
            Cow::Owned(out)
        }
    }
}

/// Runs `$body` with `$conn` bound to the connection of `$tx`, whichever
/// backend it is on. A macro, because each backend's connection is its own
/// type and the body is checked once per type.
macro_rules! on_tx {
    ($tx:expr, |$conn:ident| $body:expr) => {
        match $tx {
            Tx::Sqlite(tx) => {
                let $conn = &mut **tx;
                $body
            }
            Tx::Postgres(tx) => {
                let $conn = &mut **tx;
                $body
            }
            Tx::MariaDb(tx) => {
                let $conn = &mut **tx;
                $body
            }
        }
    };
}

use on_tx;

/// The current time as epoch microseconds in UTC, the form every time column
/// stores (doc/adr/0019).
pub fn now_micros() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is past 1970");
    i64::try_from(since_epoch.as_micros()).expect("the clock is before the year 294000")
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
