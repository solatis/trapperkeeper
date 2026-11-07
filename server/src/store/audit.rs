//! The audit trail in the relational store: appending a row. A row is a
//! record, never deleted, so no read filters on `deleted_at`.

use sqlx::AssertSqlSafe;
use uuid::Uuid;

use super::{Tx, now_micros, on_tx, sql};

/// What one request leaves in the trail.
pub struct Record<'a> {
    pub tenant_id: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub method: &'a str,
    pub route: &'a str,
    pub path_params: &'a str,
    pub status: u16,
    pub peer_address: &'a str,
    pub client_address: &'a str,
}

/// Appends `record` with a new id and now as its time. Does not commit.
///
/// # Errors
///
/// Fails when the database cannot be written.
pub async fn append(tx: &mut Tx, record: Record<'_>) -> Result<(), sqlx::Error> {
    // A version 7 UUID, so the id orders the trail by time (doc/adr/0020).
    let audit_id = Uuid::now_v7().to_string();
    let insert = sql(
        tx,
        "INSERT INTO audit (audit_id, tenant_id, user_id, method, route, path_params, status, \
         peer_address, client_address, recorded_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    );
    on_tx!(tx, |conn| {
        sqlx::query(AssertSqlSafe(insert))
            .bind(&audit_id)
            .bind(record.tenant_id)
            .bind(record.user_id)
            .bind(record.method)
            .bind(record.route)
            .bind(record.path_params)
            .bind(i32::from(record.status))
            .bind(record.peer_address)
            .bind(record.client_address)
            .bind(now_micros())
            .execute(conn)
            .await
            .map(|_| ())
    })
}
