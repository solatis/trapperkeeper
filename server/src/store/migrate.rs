//! The migrations embedded in the binary: applying them, and checking a
//! database against them before serving.

use std::fmt;

use sqlx::migrate::{AppliedMigration, Migrate, MigrateError, Migrator};
use sqlx::pool::PoolConnection;
use sqlx::sqlite::Sqlite;

use super::Store;

/// The SQLite migrations, embedded at compile time.
static SQLITE: Migrator = sqlx::migrate!("migrations/sqlite");

/// How a database differs from the binary's migrations, with the versions
/// concerned.
#[derive(Debug)]
pub enum Mismatch {
    /// Migrations the binary has and the database has not applied.
    Pending(Vec<i64>),
    /// Versions the database applied and the binary does not know.
    Unknown(Vec<i64>),
    /// Versions whose applied file differs from the binary's.
    Edited(Vec<i64>),
    /// A version whose application failed half way.
    Dirty(i64),
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Only a pending migration is fixed by migrate. The other mismatches
        // say to rebuild, because nothing is migrated backwards or repaired in
        // place (doc/principles.md 8).
        match self {
            Self::Pending(versions) => write!(
                f,
                "the database has not applied migrations {} of this binary; run `trapperkeeper migrate`",
                list(versions)
            ),
            Self::Unknown(versions) => write!(
                f,
                "the database applied migrations {} that this binary does not know; rebuild the database",
                list(versions)
            ),
            Self::Edited(versions) => write!(
                f,
                "migrations {} differ from the files the database applied; rebuild the database",
                list(versions)
            ),
            Self::Dirty(version) => write!(
                f,
                "migration {version} failed half way in the database; rebuild the database"
            ),
        }
    }
}

/// The versions as a comma-separated list.
fn list(versions: &[i64]) -> String {
    versions
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Applies the pending migrations in version order and returns how many ran.
///
/// # Errors
///
/// Fails on a database that does not match the binary, or when a migration
/// fails.
pub async fn apply(store: &Store) -> Result<usize, MigrateError> {
    let applied = tracked(store)
        .await?
        .list_applied_migrations(&SQLITE.table_name)
        .await?;
    SQLITE.run(&store.pool).await?;
    Ok(pending(&applied).len())
}

/// Compares the database with the binary's migrations without applying any.
/// Returns `None` when they match.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn check(store: &Store) -> Result<Option<Mismatch>, MigrateError> {
    // 1. open a connection with the tracking table in place
    // 2. a dirty version
    // 3. unknown versions
    // 4. edited versions
    // 5. pending migrations
    //
    // The checks run from the mismatch that most needs a rebuild to the one
    // migrate fixes, so the message names the first problem to solve.
    // 1. A database without a tracking table gets an empty one, so it reads
    //    as every migration pending.
    let mut connection = tracked(store).await?;

    // 2. A dirty row stays in the table, so it is checked before the others
    //    read it as applied.
    if let Some(version) = connection.dirty_version(&SQLITE.table_name).await? {
        return Ok(Some(Mismatch::Dirty(version)));
    }
    let applied = connection
        .list_applied_migrations(&SQLITE.table_name)
        .await?;

    // 3. A version the binary lacks means a newer binary, or another build,
    //    made this database.
    let unknown: Vec<i64> = applied
        .iter()
        .filter(|a| !SQLITE.version_exists(a.version))
        .map(|a| a.version)
        .collect();
    if !unknown.is_empty() {
        return Ok(Some(Mismatch::Unknown(unknown)));
    }

    // 4. A different checksum means the file changed after the database
    //    applied it. A down file shares its up file's version, so only the up
    //    file is compared.
    let edited: Vec<i64> = applied
        .iter()
        .filter(|a| {
            SQLITE.iter().any(|m| {
                m.version == a.version
                    && m.migration_type.is_up_migration()
                    && m.checksum != a.checksum
            })
        })
        .map(|a| a.version)
        .collect();
    if !edited.is_empty() {
        return Ok(Some(Mismatch::Edited(edited)));
    }

    // 5. Only migrations that migrate applies are left.
    let pending = pending(&applied);
    Ok((!pending.is_empty()).then_some(Mismatch::Pending(pending)))
}

/// A connection to the database, with the empty tracking table created when
/// it is missing.
async fn tracked(store: &Store) -> Result<PoolConnection<Sqlite>, MigrateError> {
    let mut connection = store.pool.acquire().await?;
    // Reading the table fails when it is absent. Creating it is the same call
    // on every backend, and asking whether it exists is not.
    connection
        .ensure_migrations_table(&SQLITE.table_name)
        .await?;
    Ok(connection)
}

/// The versions of the binary's migrations that `applied` lacks, in version
/// order.
fn pending(applied: &[AppliedMigration]) -> Vec<i64> {
    SQLITE
        .iter()
        // A down file is embedded but never applied, so counting it would
        // leave it pending forever.
        .filter(|m| m.migration_type.is_up_migration())
        .filter(|m| applied.iter().all(|a| a.version != m.version))
        .map(|m| m.version)
        .collect()
}
