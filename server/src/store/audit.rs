//! The audit trail in the relational store: appending a row and paging a
//! tenant's trail. A row is a record, never deleted, so no read filters on
//! `deleted_at`.

use sqlx::{AssertSqlSafe, FromRow};
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

/// A row as the trail lists it, with the user's name and the user's own
/// tenant's name joined when the row has a user, deleted or not.
#[derive(FromRow, Debug)]
pub struct Row {
    #[sqlx(rename = "audit_id")]
    pub id: String,
    pub user_name: Option<String>,
    pub user_tenant: Option<String>,
    pub method: String,
    pub route: String,
    pub path_params: String,
    pub status: i32,
    pub peer_address: String,
    pub client_address: String,
    pub recorded_at: i64,
}

/// The columns of `Row`, joined with the user and the user's tenant, for
/// `page` to narrow. A macro, because `concat!` takes only literals.
macro_rules! SELECT_ROW {
    () => {
        "SELECT a.audit_id, u.user_name, t.tenant_name AS user_tenant, a.method, a.route, \
         a.path_params, a.status, a.peer_address, a.client_address, a.recorded_at \
         FROM audit a \
         LEFT JOIN users u ON u.user_id = a.user_id \
         LEFT JOIN tenants t ON t.tenant_id = u.tenant_id \
         WHERE a.tenant_id = $1 AND a.audit_id < $2"
    };
}

/// An id above every version 7 UUID, so a page with no `before` starts at
/// the newest row through the same statement.
const AFTER_EVERY_ROW: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

/// The newest `limit` rows of `tenant_id` before the row `before`, or the
/// newest `limit` when `before` is `None`, narrowed to `user_id` when one is
/// given.
///
/// # Errors
///
/// Fails when the database cannot be read.
pub async fn page(
    tx: &mut Tx,
    tenant_id: &str,
    user_id: Option<&str>,
    before: Option<&str>,
    limit: u32,
) -> Result<Vec<Row>, sqlx::Error> {
    // The join on users is on the id alone and ignores deleted_at, because
    // the trail names a user deleted since, and the superadmin's row under a
    // customer tenant names a user of $tk (doc/adr/0024).
    //
    // The id is time-ordered, so it is the cursor and the order at once
    // (doc/adr/0020).
    //
    // Two statements rather than a NULL-or-equal trick on the user, because
    // the placeholders are numbered once each in order and the trick defeats
    // the user index.
    let before = before.unwrap_or(AFTER_EVERY_ROW);
    let limit = i64::from(limit);
    if let Some(user_id) = user_id {
        let select = sql(
            tx,
            concat!(
                SELECT_ROW!(),
                " AND a.user_id = $3 ORDER BY a.audit_id DESC LIMIT $4"
            ),
        );
        return on_tx!(tx, |conn| {
            sqlx::query_as(AssertSqlSafe(select))
                .bind(tenant_id)
                .bind(before)
                .bind(user_id)
                .bind(limit)
                .fetch_all(conn)
                .await
        });
    }
    let select = sql(
        tx,
        concat!(SELECT_ROW!(), " ORDER BY a.audit_id DESC LIMIT $3"),
    );
    on_tx!(tx, |conn| {
        sqlx::query_as(AssertSqlSafe(select))
            .bind(tenant_id)
            .bind(before)
            .bind(limit)
            .fetch_all(conn)
            .await
    })
}
