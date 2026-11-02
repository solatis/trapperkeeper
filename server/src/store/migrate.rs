//! The migrations embedded in the binary: applying them, and checking a
//! database against them before serving.

use sqlx::migrate::{AppliedMigration, Migrate, MigrateError, Migrator};

use super::Store;

/// The SQLite migrations, embedded at compile time.
static SQLITE: Migrator = sqlx::migrate!("migrations/sqlite");

/// Applies the pending migrations in version order and returns how many ran.
///
/// # Errors
///
/// Fails on a database that does not match the binary, or when a migration
/// fails.
pub async fn apply(store: &Store) -> Result<usize, MigrateError> {
    let applied = applied(store).await?;
    SQLITE.run(&store.pool).await?;
    Ok(pending(&applied).len())
}

/// The migrations the database has applied. Creates the empty tracking table
/// when it is missing.
async fn applied(store: &Store) -> Result<Vec<AppliedMigration>, MigrateError> {
    let mut connection = store.pool.acquire().await?;
    // Reading the table fails when it is absent. Creating it is the same call
    // on every backend, and asking whether it exists is not.
    connection
        .ensure_migrations_table(&SQLITE.table_name)
        .await?;
    connection.list_applied_migrations(&SQLITE.table_name).await
}

/// The versions of the binary's migrations that `applied` lacks, in version
/// order.
fn pending(applied: &[AppliedMigration]) -> Vec<i64> {
    SQLITE
        .iter()
        // `Migrator::run` skips down migrations, so one is never pending.
        .filter(|m| m.migration_type.is_up_migration())
        .filter(|m| applied.iter().all(|a| a.version != m.version))
        .map(|m| m.version)
        .collect()
}
