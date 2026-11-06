//! Tenants and the roles each one is created with.

use sqlx::{AssertSqlSafe, FromRow};

use super::{SYSTEM_TENANT_ID, Tx, on_tx, sql};

/// A live tenant, as listed.
#[derive(FromRow, Debug)]
pub struct Tenant {
    #[sqlx(rename = "tenant_name")]
    pub name: String,
    #[sqlx(rename = "tenant_title")]
    pub title: String,
}

/// The live tenants other than the system tenant, ordered by name.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn list(tx: &mut Tx) -> Result<Vec<Tenant>, sqlx::Error> {
    // `$tk` holds the superadmin and no customer, so it is left out.
    let select = sql(
        tx,
        "SELECT tenant_name, tenant_title FROM tenants \
         WHERE deleted_at IS NULL AND tenant_id <> $1 ORDER BY tenant_name",
    );
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(SYSTEM_TENANT_ID)
            .fetch_all(conn)
            .await
    })
}
