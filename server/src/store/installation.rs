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
    // 16 random bytes as hex. The CHECK on singleton keeps a second row out.
    let salt = rand::random::<[u8; 16]>()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    let insert = sql(
        tx,
        "INSERT INTO installation (singleton, signing_salt) VALUES (1, $1)",
    );
    on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(insert))
            .bind(&salt)
            .execute(conn)
            .await
            .map(|_| ())
    })
}

/// The installation's signing salt, or `None` before migrate has written it.
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
