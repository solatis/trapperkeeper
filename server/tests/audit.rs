//! The audit trail on the router served in this process, on each backend:
//! which requests leave rows and what they hold, paging, and a deleted user
//! still named.

mod common;

use common::{
    InProcess, SYSTEM_TENANT_ID, call, log_in, on_each_backend, on_pool, request, serve_in_process,
    superadmin_token,
};
use serde_json::{Value, json};

/// Creates the tenant `acme` with the admin `ada@example.com` as the
/// superadmin, and returns the admin's password.
fn create_acme(server: &InProcess, token: &str) -> String {
    let new = json!({ "name": "acme", "title": "Acme", "admin": "ada@example.com" });
    let response = call(server, token, "POST", "/api/v1/tenants", Some(&new));
    assert_eq!(response.status, 201, "{}", response.body);
    let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
    created["admin_password"]
        .as_str()
        .expect("a password")
        .to_owned()
}

/// The rows of the trail `query` selects on the server's database, as
/// (method, route, status, user id, tenant id, path params, peer, client).
type StoredRow = (
    String,
    String,
    i32,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
);

/// The rows of `tenant_id`'s trail in time order, read from the store, or
/// every row without a tenant when `tenant_id` is `None`.
fn stored_rows(server: &InProcess, tenant_id: Option<&str>) -> Vec<StoredRow> {
    let select = if tenant_id.is_some() {
        "SELECT method, route, status, user_id, tenant_id, path_params, peer_address, client_address \
         FROM audit WHERE tenant_id = $1 ORDER BY audit_id"
    } else {
        "SELECT method, route, status, user_id, tenant_id, path_params, peer_address, client_address \
         FROM audit WHERE tenant_id IS NULL AND $1 = 'x' ORDER BY audit_id"
    };
    let select = common::placeholders(server.database().backend, select);
    on_pool!(server.database(), |pool| {
        sqlx::query_as(sqlx::AssertSqlSafe(select))
            .bind(tenant_id.unwrap_or("x"))
            .fetch_all(&pool)
            .await
            .expect("the rows are read")
    })
}

/// The trail of `tenant` through the route, with `query` appended.
fn trail(server: &InProcess, token: &str, tenant: &str, query: &str) -> Vec<Value> {
    let path = format!("/api/v1/tenants/{tenant}/audit{query}");
    let response = call(server, token, "GET", &path, None);
    assert_eq!(response.status, 200, "{}", response.body);
    let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
    listed.as_array().expect("an array").clone()
}

/// The superadmin's login and its creation of a tenant leave rows in `$tk`'s
/// trail with the peer as the client, a GET leaves none, a request with
/// X-Forwarded-For shows it as the client beside the unchanged peer, a failed
/// login leaves a row under the named user's tenant with no user, a login
/// for an unknown user leaves a row with no tenant, and a mint is filed
/// under the user's tenant with the superadmin named as the caller.
#[test]
fn changing_requests_leave_rows_and_reads_do_not() {
    // 1. log in as the superadmin, create a tenant, list the tenants
    // 2. read $tk's rows from the store
    // 3. send a request with X-Forwarded-For
    // 4. fail a login for a known user and for an unknown one
    // 5. mint a session for the tenant's admin
    // 6. read the tenant's trail through the route
    on_each_backend(|backend| {
        let server = serve_in_process(backend);

        // 1.
        let token = superadmin_token(&server);
        create_acme(&server, &token);
        let response = call(&server, &token, "GET", "/api/v1/tenants", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);

        // 2. The login row has the tenant and no user, because the name
        //    tried is never stored; the create names the superadmin.
        let rows = stored_rows(&server, Some(SYSTEM_TENANT_ID));
        assert_eq!(rows.len(), 2, "{rows:?} on {backend}");
        let (method, route, status, user, tenant, params, peer, client) = &rows[0];
        assert_eq!(
            (method.as_str(), route.as_str(), *status),
            ("POST", "/api/v1/login", 200)
        );
        assert_eq!(
            (user.as_deref(), tenant.as_deref()),
            (None, Some(SYSTEM_TENANT_ID))
        );
        assert_eq!(
            (params.as_str(), peer.as_str(), client.as_str()),
            ("{}", "127.0.0.1", "127.0.0.1")
        );
        let (method, route, status, user, _, params, _, _) = &rows[1];
        assert_eq!(
            (method.as_str(), route.as_str(), *status),
            ("POST", "/api/v1/tenants", 201)
        );
        assert!(
            user.is_some(),
            "the create names the superadmin on {backend}"
        );
        assert_eq!(params, "{}");

        // 3. The header is trusted as received, which is the one-proxy rule;
        //    the refused create is recorded with its 409.
        let bearer = format!("Bearer {token}");
        let new = json!({ "name": "acme", "title": "Acme", "admin": "ada@example.com" });
        let response = request(
            server.addr,
            "POST",
            "/api/v1/tenants",
            &[
                ("Authorization", &bearer),
                ("X-Forwarded-For", "203.0.113.7"),
            ],
            Some(&new.to_string()),
        );
        assert_eq!(response.status, 409, "{} on {backend}", response.body);
        let rows = stored_rows(&server, Some(SYSTEM_TENANT_ID));
        let (_, _, status, _, _, _, peer, client) = &rows[2];
        assert_eq!(
            (*status, peer.as_str(), client.as_str()),
            (409, "127.0.0.1", "203.0.113.7")
        );

        // 4. The row of a login for an unknown user has no tenant, so no
        //    admin reads it, and a store-level read sees it.
        let wrong = json!({ "tenant": "acme", "name": "ada@example.com", "password": "wrong" });
        let response = request(
            server.addr,
            "POST",
            "/api/v1/login",
            &[],
            Some(&wrong.to_string()),
        );
        assert_eq!(response.status, 401, "on {backend}");
        let unknown = json!({ "tenant": "acme", "name": "nobody@example.com", "password": "x" });
        let response = request(
            server.addr,
            "POST",
            "/api/v1/login",
            &[],
            Some(&unknown.to_string()),
        );
        assert_eq!(response.status, 401, "on {backend}");
        let rows = stored_rows(&server, None);
        assert_eq!(rows.len(), 1, "{rows:?} on {backend}");
        assert_eq!((rows[0].1.as_str(), rows[0].2), ("/api/v1/login", 401));

        // 5.
        let response = call(
            &server,
            &token,
            "POST",
            "/api/v1/tenants/acme/users/ada@example.com/session",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);

        // 6. Newest first: the mint, filed under acme with the superadmin
        //    and its own tenant named, then the failed login with no user.
        let listed = trail(&server, &token, "acme", "");
        assert_eq!(listed.len(), 2, "{listed:?} on {backend}");
        assert_eq!(
            listed[0]["route"],
            "/api/v1/tenants/{tenant}/users/{name}/session"
        );
        assert_eq!(listed[0]["status"], 200);
        assert_eq!(listed[0]["user"], "admin");
        assert_eq!(listed[0]["user_tenant"], "$tk");
        assert_eq!(
            listed[0]["path_params"],
            json!({ "name": "ada@example.com", "tenant": "acme" })
        );
        assert_eq!(listed[0]["client_address"], "127.0.0.1");
        assert_eq!(listed[1]["route"], "/api/v1/login");
        assert_eq!(listed[1]["status"], 401);
        assert_eq!(listed[1]["user"], Value::Null);
    });
}

/// Paging: more rows than the limit, newest first, `before` continues where
/// the page ended, and `user` narrows the page to that user's rows.
#[test]
fn the_trail_pages_newest_first() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let password = create_acme(&server, &token);
        let (ada, _) = log_in(&server, "acme", "ada@example.com", &password);
        // Five creates by the admin, after the admin's own login row.
        for n in 0..5 {
            let new = json!({ "name": format!("u{n}@example.com"), "role": "observer" });
            let response = call(
                &server,
                &ada,
                "POST",
                "/api/v1/tenants/acme/users",
                Some(&new),
            );
            assert_eq!(response.status, 201, "{} on {backend}", response.body);
        }

        let first = trail(&server, &ada, "acme", "?limit=4");
        assert_eq!(first.len(), 4, "{first:?} on {backend}");
        let ids: Vec<&str> = first
            .iter()
            .map(|r| r["id"].as_str().expect("an id"))
            .collect();
        assert!(ids.windows(2).all(|w| w[0] > w[1]), "{ids:?} on {backend}");
        assert_eq!(first[0]["route"], "/api/v1/tenants/{tenant}/users");

        let second = trail(
            &server,
            &ada,
            "acme",
            &format!("?limit=4&before={}", ids[3]),
        );
        assert_eq!(second.len(), 2, "{second:?} on {backend}");
        assert!(second[0]["id"].as_str().expect("an id") < ids[3]);
        assert_eq!(second[1]["route"], "/api/v1/login");

        let narrowed = trail(&server, &ada, "acme", "?user=Ada@Example.com");
        assert_eq!(narrowed.len(), 5, "{narrowed:?} on {backend}");
        assert!(narrowed.iter().all(|r| r["user"] == "ada@example.com"));
        let none = trail(&server, &ada, "acme", "?user=nobody@example.com");
        assert_eq!(none, [] as [Value; 0]);
    });
}

/// A row for a user deleted since still shows the name, because the join
/// ignores `deleted_at`, and the user filter finds the deleted user.
#[test]
fn the_trail_names_a_deleted_user() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let password = create_acme(&server, &token);
        let (ada, _) = log_in(&server, "acme", "ada@example.com", &password);
        let new = json!({ "name": "bob@example.com", "role": "observer" });
        let response = call(
            &server,
            &ada,
            "POST",
            "/api/v1/tenants/acme/users",
            Some(&new),
        );
        assert_eq!(response.status, 201, "{} on {backend}", response.body);
        let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
        let (bob, _) = log_in(
            &server,
            "acme",
            "bob@example.com",
            created["password"].as_str().expect("a password"),
        );
        // A logout is bob's one changing request with bob as the caller.
        let response = call(&server, &bob, "POST", "/api/v1/logout", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let response = call(
            &server,
            &ada,
            "DELETE",
            "/api/v1/tenants/acme/users/bob@example.com",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);

        let rows = trail(&server, &ada, "acme", "?user=bob@example.com");
        assert_eq!(rows.len(), 1, "{rows:?} on {backend}");
        assert_eq!(rows[0]["route"], "/api/v1/logout");
        assert_eq!(rows[0]["user"], "bob@example.com");
        assert_eq!(rows[0]["user_tenant"], "acme");
    });
}
