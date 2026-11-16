//! What the tables that migrate builds refuse, on each backend the
//! environment names.

mod common;

use common::{
    SYSTEM_TENANT_ID, TestDatabase, on_each_backend, on_pool, placeholders, set_up_database,
};
use sqlx::AssertSqlSafe;

/// On every backend, the audit table that migrate builds takes a row for a
/// tenant that exists and refuses a row for a tenant that does not.
#[test]
fn an_audit_row_names_an_existing_tenant() {
    on_each_backend(|backend| {
        let database = TestDatabase::fresh(backend);
        set_up_database(&database);
        let insert = placeholders(
            backend,
            "INSERT INTO audit (audit_id, tenant_id, method, route, path_params, status, \
             peer_address, client_address, recorded_at) \
             VALUES ($1, $2, 'POST', '/api/v1/tenants', '{}', 200, '127.0.0.1', '127.0.0.1', 0)",
        );
        // Whether the audit table takes a row with this id for this tenant.
        let takes = |audit_id: &'static str, tenant_id: &'static str| {
            on_pool!(database, |pool| {
                sqlx::query(AssertSqlSafe(insert.clone()))
                    .bind(audit_id)
                    .bind(tenant_id)
                    .execute(&pool)
                    .await
                    .is_ok()
            })
        };
        // The set-up created the system tenant, so a tenant with this id
        // exists.
        assert!(takes("a", SYSTEM_TENANT_ID), "{backend}");
        // Every tenant id is a UUID, so no tenant has this id.
        assert!(!takes("b", "no such tenant"), "{backend}");
    });
}
