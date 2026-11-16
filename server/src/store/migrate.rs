//! The migrations of each backend, embedded in the binary. The module
//! applies them, and it checks a database against them before serve or
//! reset-superadmin uses it.

use std::fmt;

use sqlx::migrate::{AppliedMigration, Migrate, MigrateError, Migrator};
use sqlx::pool::PoolConnection;
use sqlx::{Database, Pool};

use super::{Store, installation};

// The migrations of each backend, embedded at compile time.
static SQLITE: Migrator = sqlx::migrate!("migrations/sqlite");
static POSTGRES: Migrator = sqlx::migrate!("migrations/postgres");
static MARIADB: Migrator = sqlx::migrate!("migrations/mariadb");

/// How a database differs from the binary's migrations, with the versions
/// concerned. A command fixes `Empty` and `Pending`. `Unknown`, `Edited` and
/// `Dirty` need a rebuilt database, because the project migrates nothing
/// backwards and repairs nothing in place (ADR-0019).
#[derive(Debug)]
pub enum Mismatch {
    /// The database has applied no migration.
    Empty,
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
        match self {
            Self::Empty => f.write_str(
                "the database is empty; `trapperkeeper serve` sets it up on its first start, \
                 and `trapperkeeper migrate` applies the migrations",
            ),
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

/// Applies the pending migrations in version order, writes the installation's
/// signing salt when it has none, and returns how many migrations ran.
///
/// # Errors
///
/// Fails on a database with a dirty, an unknown or an edited migration, when
/// a migration fails, or when writing the signing salt fails. A pending or an
/// empty database does not fail, because `apply` migrates it.
pub async fn apply(store: &Store) -> Result<usize, MigrateError> {
    let count = match store {
        Store::Sqlite(pool) => apply_with(&SQLITE, pool).await?,
        Store::Postgres(pool) => apply_with(&POSTGRES, pool).await?,
        Store::MariaDb(pool) => apply_with(&MARIADB, pool).await?,
    };
    // `apply` writes the salt here and not in a migration, so every database
    // that applied the migrations has a salt of its own. The migrate command
    // and the first serve both run `apply`.
    let mut tx = store.begin().await?;
    installation::ensure_salt(&mut tx).await?;
    tx.commit().await?;
    Ok(count)
}

/// Compares the database with the binary's migrations and applies none of
/// them. It creates an empty tracking table when the database has none.
/// Returns `None` when they match.
///
/// # Errors
///
/// Fails when the tracking table cannot be created or read.
pub async fn check(store: &Store) -> Result<Option<Mismatch>, MigrateError> {
    match store {
        Store::Sqlite(pool) => check_with(&SQLITE, pool).await,
        Store::Postgres(pool) => check_with(&POSTGRES, pool).await,
        Store::MariaDb(pool) => check_with(&MARIADB, pool).await,
    }
}

/// Applies the migrations of `migrator` that the database has not applied,
/// and returns how many ran. Fails as `Migrator::run` fails.
async fn apply_with<D: Database>(migrator: &Migrator, pool: &Pool<D>) -> Result<usize, MigrateError>
where
    D::Connection: Migrate,
{
    // `Migrator::run` reports no count. A successful run applies every
    // migration that was pending before it, so the list taken before the run
    // gives the count, unless another process migrates the database between
    // the two.
    let applied = tracked(migrator, pool)
        .await?
        .list_applied_migrations(&migrator.table_name)
        .await?;
    // The connection of `tracked` was a temporary of the statement above, so
    // the pool has it back before `run` takes one.
    migrator.run(pool).await?;
    Ok(pending(migrator, &applied).len())
}

/// Returns the first mismatch between the database and `migrator`, or `None`
/// when they match. Fails when the tracking table cannot be created or read.
async fn check_with<D: Database>(
    migrator: &Migrator,
    pool: &Pool<D>,
) -> Result<Option<Mismatch>, MigrateError>
where
    D::Connection: Migrate,
{
    // The function reads the tracking table and tests it against a chain of
    // mismatches, and it returns the first that applies. The place of three
    // cases matters: `Dirty` comes first, `Empty` comes before every check
    // of the applied rows, and `Pending` comes last. The note at each of
    // them says why.

    // A database without a tracking table gets an empty one here, so it
    // reads as `Empty` below.
    let mut connection = tracked(migrator, pool).await?;

    // `Dirty` comes first, because the dirty row stays in the tracking table,
    // and the list read after it holds that row as applied.
    if let Some(version) = connection.dirty_version(&migrator.table_name).await? {
        return Ok(Some(Mismatch::Dirty(version)));
    }
    let applied = connection
        .list_applied_migrations(&migrator.table_name)
        .await?;

    // An empty table means a new database, which the first serve sets up, so
    // `Empty` is reported where `Pending` would name every migration. With
    // no applied row, the checks for `Unknown` and `Edited` below could find
    // nothing. The check reads the tracking table alone, so a database that
    // holds other tables beside an empty tracking table counts as new too.
    if applied.is_empty() {
        return Ok(Some(Mismatch::Empty));
    }

    // A version that the binary lacks means that a newer binary or another
    // build made this database. `version_exists` also counts a down file,
    // and the migration directories hold none (ADR-0019).
    let unknown: Vec<i64> = applied
        .iter()
        .filter(|a| !migrator.version_exists(a.version))
        .map(|a| a.version)
        .collect();
    if !unknown.is_empty() {
        return Ok(Some(Mismatch::Unknown(unknown)));
    }

    // A different checksum means that the migration file changed after the
    // database applied it.
    let edited: Vec<i64> = applied
        .iter()
        .filter(|a| {
            migrator.iter().any(|m| {
                m.version == a.version
                    // A down file shares the version of its up file, so the
                    // filter compares only the up file.
                    && m.migration_type.is_up_migration()
                    && m.checksum != a.checksum
            })
        })
        .map(|a| a.version)
        .collect();
    if !edited.is_empty() {
        return Ok(Some(Mismatch::Edited(edited)));
    }

    // `Pending` comes last, because it is the mismatch that
    // `trapperkeeper migrate` fixes. `Migrator::run` refuses a database that
    // is dirty, holds an unknown version or holds an edited one, so the
    // rebuild cases above must be reported first.
    let pending = pending(migrator, &applied);
    Ok((!pending.is_empty()).then_some(Mismatch::Pending(pending)))
}

/// A connection to the database, with the empty tracking table created when
/// it is missing.
async fn tracked<D: Database>(
    migrator: &Migrator,
    pool: &Pool<D>,
) -> Result<PoolConnection<D>, MigrateError>
where
    D::Connection: Migrate,
{
    let mut connection = pool.acquire().await?;
    // Reading the table fails when it is absent. Creating it is the same call
    // on every backend, and asking whether it exists is not.
    connection
        .ensure_migrations_table(&migrator.table_name)
        .await?;
    Ok(connection)
}

/// The versions of `migrator`'s migrations that `applied` lacks, in version
/// order.
fn pending(migrator: &Migrator, applied: &[AppliedMigration]) -> Vec<i64> {
    migrator
        .iter()
        // A down file is embedded but never applied, so counting it would
        // leave it pending forever.
        .filter(|m| m.migration_type.is_up_migration())
        .filter(|m| applied.iter().all(|a| a.version != m.version))
        .map(|m| m.version)
        .collect()
}
