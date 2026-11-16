//! The audit trail in the relational store. The file appends a row and
//! pages a tenant's trail. Nothing deletes an audit row, so the audit table
//! has no `deleted_at`.

use sqlx::{AssertSqlSafe, FromRow};
use uuid::Uuid;

use super::{Tx, now_micros, on_tx, sql};

/// What one changing request leaves in the trail. Each field fills the
/// column of the same name in `audit`, and the comments in
/// `server/migrations/sqlite/0005_audit.sql` say what each column holds and
/// when it is NULL. `append` adds the id and the time.
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

/// Inserts `record` with a new id and the current time as `recorded_at`. The
/// caller commits the transaction.
///
/// # Errors
///
/// Fails when the database cannot be written.
pub async fn append(tx: &mut Tx, record: Record<'_>) -> Result<(), sqlx::Error> {
    // The id is a version 7 UUID, whose text sorts by creation time, so
    // ordering by `audit_id` orders the trail by time (ADR-0020).
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

/// Expands to the SELECT of the columns of `Row`, joined with the user and
/// the user's own tenant. It ends in a WHERE clause that binds `$1` to the
/// tenant id and `$2` to the cursor, and it keeps the rows whose id sorts
/// below `$2`. Those are the older rows, because the ids are time-ordered
/// (ADR-0020). A caller appends its own conditions from `$3` on, then the
/// order and the limit. A macro, because `concat!` accepts only literals, so
/// a `const` would not work.
macro_rules! SELECT_ROWS_BEFORE {
    () => {
        // Neither LEFT JOIN filters on `deleted_at`, so the trail still names
        // a user deleted since (ADR-0024). The join on users matches on
        // `user_id` alone, because a row filed under a customer tenant can
        // name the superadmin, who is a user of `$tk` (ADR-0031).
        "SELECT a.audit_id, u.user_name, t.tenant_name AS user_tenant, a.method, a.route, \
         a.path_params, a.status, a.peer_address, a.client_address, a.recorded_at \
         FROM audit a \
         LEFT JOIN users u ON u.user_id = a.user_id \
         LEFT JOIN tenants t ON t.tenant_id = u.tenant_id \
         WHERE a.tenant_id = $1 AND a.audit_id < $2"
    };
}

/// The all-f UUID, whose text sorts above every id the store generates.
/// `page` uses it as the cursor when the caller gives no `before`, so the
/// first page runs the same statement as the pages after it and starts at
/// the newest row.
const AFTER_EVERY_ROW: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";

/// Returns up to `limit` rows of `tenant_id`, newest first, whose ids sort
/// below `before`, or the newest rows when `before` is `None`. When `user_id`
/// is given, it returns only that user's rows.
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
    let before = before.unwrap_or(AFTER_EVERY_ROW);
    // sqlx encodes no unsigned integer for Postgres, and `i64` holds every
    // `u32`, which `i32` does not.
    let limit = i64::from(limit);
    // `page` runs a separate statement when a user is given. One statement
    // with `$3 IS NULL OR a.user_id = $3` would use `$3` twice, and each
    // placeholder number appears once, in ascending order, because
    // MariaDB's `?` binds by position (ADR-0019). The OR form would also
    // defeat the index `audit_tenant_user_time`.
    if let Some(user_id) = user_id {
        let select = sql(
            tx,
            concat!(
                SELECT_ROWS_BEFORE!(),
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
        concat!(SELECT_ROWS_BEFORE!(), " ORDER BY a.audit_id DESC LIMIT $3"),
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
