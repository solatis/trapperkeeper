//! The relational store: the control-plane data, in SQLite, Postgres or
//! MariaDB. The rules every table and query follow are in this directory's
//! `AGENTS.md`.

pub mod audit;
pub mod installation;
pub mod migrate;
pub mod tenants;
pub mod users;

use std::borrow::Cow;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::migrate::MigrateDatabase;
use sqlx::mysql::{MySql, MySqlPool};
use sqlx::postgres::{PgPool, Postgres};
use sqlx::sqlite::{Sqlite, SqliteConnectOptions, SqlitePool};
use sqlx::{Database, Executor, Transaction};

/// The id of the system tenant `$tk`, which the first migration seeds. `$tk`
/// holds the superadmin and is no customer tenant (ADR-0028).
pub const SYSTEM_TENANT_ID: &str = "00000000-0000-7000-8000-000000000000";

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

/// Returns `statement` with its placeholders in the form the backend of `tx`
/// expects. MariaDB gets `?` in place of `$1`, `$2` and so on, and SQLite and
/// Postgres get the statement unchanged. A statement uses each placeholder
/// number once and in ascending order, because on MariaDB the values bind in
/// the order the placeholders appear and the numbers are dropped.
fn sql(tx: &Tx, statement: &'static str) -> Cow<'static, str> {
    match tx {
        Tx::Sqlite(_) | Tx::Postgres(_) => Cow::Borrowed(statement),
        // MariaDB has no numbered placeholder, only `?`, so the arm rewrites
        // each one (ADR-0019).
        Tx::MariaDb(_) => {
            let mut out = String::with_capacity(statement.len());
            let mut chars = statement.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '$' && chars.peek().is_some_and(char::is_ascii_digit) {
                    out.push('?');
                    // A placeholder can have several digits, as `$10` does,
                    // and the whole number becomes one `?`. The loop does
                    // not know SQL quoting, so it would also rewrite a `$`
                    // and a digit inside a quoted literal, and no statement
                    // of the store holds one.
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

/// Runs `$body` with `$conn` bound to the connection of `$tx`, a `&mut Tx`,
/// whichever backend it is on.
macro_rules! on_tx {
    ($tx:expr, |$conn:ident| $body:expr) => {
        // The three arms repeat the body, because each binds `$conn` to a
        // connection of a different type, so the compiler checks the body
        // once for each type.
        match $tx {
            Tx::Sqlite(tx) => {
                // `&mut **tx` reaches the backend's connection inside the
                // sqlx `Transaction`, which is the executor that the body's
                // queries run on.
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

// `use on_tx` makes the macro an item of this module, so the submodules
// import it by path as `super::on_tx`. The `mod` lines above come before the
// macro, so its textual scope does not reach them.
use on_tx;

/// The current time as epoch microseconds in UTC, the form every time column
/// stores (ADR-0019).
pub fn now_micros() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is past 1970");
    i64::try_from(since_epoch.as_micros()).expect("the clock is before the year 294000")
}

/// The kind of database server, which the scheme of the URL chooses.
enum Backend {
    Sqlite,
    Postgres,
    MariaDb,
}

/// The backend `url`'s scheme names.
///
/// # Errors
///
/// Fails when no driver among SQLite, Postgres and MySQL lists the scheme of
/// `url`. The message lists the schemes that the drivers accept.
fn backend(url: &str) -> Result<Backend, sqlx::Error> {
    // A URL without a colon, such as a bare file path, gets the empty scheme,
    // which no driver lists, so the function returns the error below.
    let scheme = url.split_once(':').map_or("", |(scheme, _)| scheme);
    // The table takes each driver's own `URL_SCHEMES`, so the store accepts
    // the schemes that the driver parses.
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

/// Opens the store at `url` without creating the database.
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

/// Opens the store at `url`, creating the database when it is missing.
///
/// # Errors
///
/// Fails when the URL does not parse, or when the database cannot be created
/// or opened. On Postgres and MariaDB it fails when the database is missing
/// and the user of the URL may not create databases.
pub async fn create(url: &str) -> Result<Store, sqlx::Error> {
    match backend(url)? {
        // `mode=rwc` in a SQLite URL also creates a missing file. `create`
        // asks sqlx for that through `open_sqlite` and leaves the URL alone,
        // so migrate and serve take the same URL.
        Backend::Sqlite => open_sqlite(url, true).await,
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

/// Whether the database `url` names exists.
///
/// # Errors
///
/// Fails when the URL does not parse or the server cannot be asked.
pub async fn exists(url: &str) -> Result<bool, sqlx::Error> {
    match backend(url)? {
        Backend::Sqlite => Sqlite::database_exists(url).await,
        Backend::Postgres => Postgres::database_exists(url).await,
        Backend::MariaDb => MySql::database_exists(url).await,
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
    // Two processes that create the same missing database at once can both
    // see it missing. The second `create_database` then fails, and so does
    // `create`.
    if !D::database_exists(url).await? {
        D::create_database(url).await?;
    }
    Ok(())
}
