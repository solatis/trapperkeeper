//! Opening the database for each command. The module opens or creates the
//! database, checks it against the binary's migrations and sets up an empty
//! one with its superadmin. Each command accepts a different state of the
//! database, so each has its own entry: `create` for migrate, `open_or_set_up`
//! for serve and `open_checked` for reset-superadmin.

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

/// Why a command could not get its database. Each command prints the
/// `Display` text on stderr, and after an `Open` error reset-superadmin adds a
/// hint about creating the database.
#[derive(Debug)]
pub enum Error {
    /// The database does not open.
    Open(sqlx::Error),
    /// The URL does not parse, or the database could not be created or did
    /// not open after its creation.
    Create(sqlx::Error),
    /// The database could not be compared with the migrations.
    Check(MigrateError),
    /// The database differs from the migrations.
    Mismatch(Mismatch),
    /// A migration failed, the salt of the token keys could not be written,
    /// or the applied migrations do not match the binary's.
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
/// it did.
///
/// # Errors
///
/// Fails when the database cannot be created or opened, when a migration or
/// the signing salt fails, when the applied migrations do not match the
/// binary's, or when the superadmin cannot be created.
pub async fn create(url: &str) -> Result<(Store, SetUp), Error> {
    let store = store::create(url).await.map_err(Error::Create)?;
    let set_up = set_up(&store).await?;
    Ok((store, set_up))
}

/// Opens the database, creating it only when it does not exist, and checks it
/// against the migrations. It sets up an empty database and returns what the
/// set-up did, and it returns `None` for a database that matches.
///
/// # Errors
///
/// Fails when the database cannot be opened or created, when it cannot be
/// compared with the migrations, when it differs from them other than by
/// being empty, or when setting it up fails.
pub async fn open_or_set_up(url: &str) -> Result<(Store, Option<SetUp>), Error> {
    // The function sets up a database that has applied no migration, and it
    // refuses every other mismatch.
    //
    // 1. We open the database, and create it only when the open fails and the
    //    database is known not to exist. Opening first lets a serve whose
    //    database user may not create databases open an existing one.
    // 2. We compare the database with the migrations, set up an empty one and
    //    refuse any other mismatch. serve migrates only an empty database,
    //    because rolling deploys rule out migrating on every serve (ADR-0028).

    // 1. open, create when missing
    let store = match store::open(url).await {
        Ok(store) => store,
        // serve never creates a database that may exist, so when it cannot
        // find out whether one exists, it refuses with the error of the open.
        Err(error) if !matches!(store::exists(url).await, Ok(false)) => {
            return Err(Error::Open(error));
        }
        Err(_) => store::create(url).await.map_err(Error::Create)?,
    };

    // 2. check, set up an empty database
    match migrate::check(&store).await.map_err(Error::Check)? {
        None => Ok((store, None)),
        // `Empty` means that the tracking table lists no applied migration,
        // as in a new database.
        Some(Mismatch::Empty) => {
            let set_up = set_up(&store).await?;
            Ok((store, Some(set_up)))
        }
        Some(mismatch) => Err(Error::Mismatch(mismatch)),
    }
}

/// Opens the database without creating it and checks it against the
/// migrations.
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
    // The superadmin comes after the migrations, which seed the system tenant
    // `$tk` and its superadmin role.
    let password = superadmin::ensure(store).await.map_err(Error::Superadmin)?;
    Ok(SetUp { applied, password })
}
