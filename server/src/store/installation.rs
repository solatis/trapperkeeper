//! Facts about this installation that every instance shares: the salt the
//! signing keys are stretched with.

use std::fmt::Write as _;

use sqlx::AssertSqlSafe;

use super::{Tx, on_tx, sql};

/// Writes a random signing salt unless the installation has one.
///
/// # Errors
///
/// Fails when the database cannot be read or written.
pub async fn ensure_salt(tx: &mut Tx) -> Result<(), sqlx::Error> {
    if salt(tx).await?.is_some() {
        return Ok(());
    }
    // The salt is 16 random bytes written as 32 hex characters, which the TEXT
    // column holds. The size of 16 bytes is a choice that no measurement
    // backs.
    let salt = rand::random::<[u8; 16]>()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            // Writing to a `String` cannot fail, so the result is dropped.
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    // The insert always writes singleton 1, so the primary key on `singleton`
    // refuses a second row, and the match below receives that refusal as a
    // unique violation.
    let insert = sql(
        tx,
        "INSERT INTO installation (singleton, signing_salt) VALUES (1, $1)",
    );
    let inserted = on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(insert))
            .bind(&salt)
            .execute(conn)
            .await
            // Each backend's `execute` returns its own result type, and
            // dropping it gives the three arms of `on_tx` one type.
            .map(|_| ())
    });
    match inserted {
        // Another instance wrote its salt between the read and the insert,
        // and that salt is the installation's. On Postgres the violation has
        // also aborted the transaction of `tx`. A later commit of it then
        // ends as a rollback without an error, and whatever else the
        // transaction wrote is lost.
        Err(sqlx::Error::Database(database)) if database.is_unique_violation() => Ok(()),
        inserted => inserted,
    }
}

/// The installation's signing salt, or `None` when the installation table
/// holds no row. That is the state between the migrations and the write of
/// the salt, both of which `migrate::apply` does.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn salt(tx: &mut Tx) -> Result<Option<String>, sqlx::Error> {
    let select = sql(
        tx,
        "SELECT signing_salt FROM installation WHERE singleton = 1",
    );
    on_tx!(tx, |conn| {
        sqlx::query_scalar(AssertSqlSafe(select))
            .fetch_optional(conn)
            .await
    })
}
