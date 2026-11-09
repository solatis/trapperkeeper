//! Opening the database for a command: open or create it, check it against
//! the binary's migrations, and set up an empty one with its superadmin. Each
//! command has its own entry, because each accepts a different state of the
//! database.

use std::fmt;

use sqlx::migrate::MigrateError;

use crate::store::migrate::{self, Mismatch};
use crate::store::{self, Store};
use crate::superadmin;

/// What setting up a database did: how many migrations ran, and the
/// superadmin's one-time password when this set-up created it.
#[derive(Debug)]
pub struct SetUp {
    pub applied: usize,
    pub password: Option<String>,
}

/// Why a command could not get its database. `Display` is the text the
/// command prints.
#[derive(Debug)]
pub enum Error {
    /// The database does not open.
    Open(sqlx::Error),
    /// The database could not be created.
    Create(sqlx::Error),
    /// The database could not be compared with the migrations.
    Check(MigrateError),
    /// The database differs from the migrations.
    Mismatch(Mismatch),
    /// A migration or the signing salt failed.
    Migrate(MigrateError),
    /// The superadmin could not be created.
    Superadmin(sqlx::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open(error) | Self::Create(error) | Self::Superadmin(error) => {
                write!(f, "{error}")
            }
            Self::Check(error) | Self::Migrate(error) => write!(f, "{error}"),
            Self::Mismatch(mismatch) => write!(f, "{mismatch}"),
        }
    }
}

impl std::error::Error for Error {}

/// Creates the database when it is missing, applies the pending migrations,
/// creates the superadmin when it is missing, and returns the store with what
/// it did. For migrate.
///
/// # Errors
///
/// Fails when the database cannot be created or opened, when a migration
/// fails, or when the superadmin cannot be created.
pub async fn create(url: &str) -> Result<(Store, SetUp), Error> {
    let store = store::create(url).await.map_err(Error::Create)?;
    let set_up = set_up(&store).await?;
    Ok((store, set_up))
}

/// Opens the database, creating it only when it does not exist, and checks it
/// against the migrations. An empty database is set up and the set-up
/// returned, and a database that matches returns `None`. For serve.
///
/// # Errors
///
/// Fails when the database cannot be opened or created, when it cannot be
/// compared with the migrations, when it differs from them other than by
/// being empty, or when setting it up fails.
pub async fn open_or_set_up(url: &str) -> Result<(Store, Option<SetUp>), Error> {
    // 1. open, and create only when nothing exists at the URL
    // 2. check against the migrations
    // 3. set up an empty database

    // 1. Open first, so a serve whose user may not create databases still
    //    opens an existing one.
    let store = match store::open(url).await {
        Ok(store) => store,
        // A database that exists, or whose existence cannot be asked, is
        // refused with the open's error.
        Err(error) if !matches!(store::exists(url).await, Ok(false)) => {
            return Err(Error::Open(error));
        }
        Err(_) => store::create(url).await.map_err(Error::Create)?,
    };

    // 2. and 3.
    match migrate::check(&store).await.map_err(Error::Check)? {
        None => Ok((store, None)),
        Some(Mismatch::Empty) => {
            let set_up = set_up(&store).await?;
            Ok((store, Some(set_up)))
        }
        Some(mismatch) => Err(Error::Mismatch(mismatch)),
    }
}

/// Opens the database without creating it and checks it against the
/// migrations. For reset-superadmin.
///
/// # Errors
///
/// Fails when the database does not open, cannot be compared with the
/// migrations, or differs from them.
pub async fn open_checked(url: &str) -> Result<Store, Error> {
    let store = store::open(url).await.map_err(Error::Open)?;
    match migrate::check(&store).await.map_err(Error::Check)? {
        None => Ok(store),
        Some(mismatch) => Err(Error::Mismatch(mismatch)),
    }
}

/// Applies the migrations and creates the superadmin when it is missing.
async fn set_up(store: &Store) -> Result<SetUp, Error> {
    let applied = migrate::apply(store).await.map_err(Error::Migrate)?;
    // After the migrations, because they seed `$tk` and its role.
    let password = superadmin::ensure(store).await.map_err(Error::Superadmin)?;
    Ok(SetUp { applied, password })
}
