//! The audit trail on the router served in this process, on each backend.
//! The tests check which requests leave rows and what a row holds, how the
//! trail pages, and that a row still names a deleted user.

mod common;

use common::{
    ADMIN, InProcess, SYSTEM_TENANT, SYSTEM_TENANT_ID, call, log_in, on_each_backend, on_pool,
    request, serve_in_process, superadmin_token,
};
use serde_json::{Value, json};

/// With the superadmin's `token`, creates the tenant `acme` whose admin is
/// `ada@example.com`, and returns the admin's one-time password. Panics unless
/// the create answers 201.
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

/// One audit row as `stored_rows` reads it: method, route, status, user id,
/// tenant id, path parameters as JSON, peer address and client address.
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
    // An audit id is a version 7 UUID, so the order of the ids is the order
    // of the writes.
    let select = if tenant_id.is_some() {
        "SELECT method, route, status, user_id, tenant_id, path_params, peer_address, client_address \
         FROM audit WHERE tenant_id = $1 ORDER BY audit_id"
    } else {
        // `$1 = 'x'` is always true and gives this statement the placeholder
        // of the other, so both take the one bind below.
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

/// GETs `/api/v1/tenants/{tenant}/audit` with `query` appended, as `token`,
/// and returns the listed rows. Panics unless the route answers 200 with a
/// JSON array.
fn trail(server: &InProcess, token: &str, tenant: &str, query: &str) -> Vec<Value> {
    let path = format!("/api/v1/tenants/{tenant}/audit{query}");
    let response = call(server, token, "GET", &path, None);
    assert_eq!(response.status, 200, "{}", response.body);
    let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
    listed.as_array().expect("an array").clone()
}

/// Each changing request leaves one row with its method, route, status,
/// caller, tenant, peer and client, and a GET leaves none. A login and the
/// superadmin's mint of a session are filed under the tenant of the user they
/// name.
#[test]
fn changing_requests_leave_rows_and_reads_do_not() {
    // The test sends requests as the superadmin and as anonymous callers, and
    // checks the rows after each set.
    //
    // 1. We log in as the superadmin, create `acme` and list the tenants. The
    //    login and the create leave rows under `$tk`, and the GET none.
    // 2. We read the rows of `$tk` from the store, because the list route
    //    answers `$tk` as an unknown tenant.
    // 3. We repeat the create with `X-Forwarded-For`. Its 409 still leaves a
    //    row, whose client is the header's address, as the middleware assumes
    //    one proxy (ADR-0031).
    // 4. We fail a login for ada and one for an unknown user. Only the
    //    unknown user's row has no tenant, so the test reads it from the
    //    store.
    // 5. We mint a session for ada, which is filed under her tenant.
    // 6. We read the trail of `acme` through the route.
    on_each_backend(|backend| {
        let server = serve_in_process(backend);

        // 1. log in, create a tenant, list the tenants
        let token = superadmin_token(&server);
        create_acme(&server, &token);
        let response = call(&server, &token, "GET", "/api/v1/tenants", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);

        // 2. read the rows of $tk
        let rows = stored_rows(&server, Some(SYSTEM_TENANT_ID));
        assert_eq!(rows.len(), 2, "{rows:?} on {backend}");
        let (method, route, status, user, tenant, params, peer, client) = &rows[0];
        assert_eq!(
            (method.as_str(), route.as_str(), *status),
            ("POST", "/api/v1/login", 200)
        );
        // A login row names no user, only the tenant of the named one.
        assert_eq!(
            (user.as_deref(), tenant.as_deref()),
            (None, Some(SYSTEM_TENANT_ID))
        );
        // Without `X-Forwarded-For` the client is the peer.
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

        // 3. create again behind a proxy
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

        // 4. fail two logins
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
        let (_, route, status, ..) = &rows[0];
        assert_eq!((route.as_str(), *status), ("/api/v1/login", 401));

        // 5. mint a session for ada
        let response = call(
            &server,
            &token,
            "POST",
            "/api/v1/tenants/acme/users/ada@example.com/session",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);

        // 6. read the trail of acme
        // The creates sit under `$tk`, and the unknown user's login has no
        // tenant, so the trail holds the mint and ada's login, newest first.
        let listed = trail(&server, &token, "acme", "");
        assert_eq!(listed.len(), 2, "{listed:?} on {backend}");
        assert_eq!(
            listed[0]["route"],
            "/api/v1/tenants/{tenant}/users/{name}/session"
        );
        assert_eq!(listed[0]["status"], 200);
        assert_eq!(listed[0]["user"], ADMIN);
        assert_eq!(listed[0]["user_tenant"], SYSTEM_TENANT);
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

/// The route lists the trail newest first, `before` continues after the last
/// row of a page, and `user` narrows the page to that user's rows.
#[test]
fn the_trail_pages_newest_first() {
    // The test fills the trail of `acme` with six rows and reads them as two
    // pages of at most four, and narrowed to one user.
    //
    // 1. We create `acme` and log ada in, whose login is the first row of
    //    its trail and names no user.
    // 2. Ada creates five users, so the trail holds six rows.
    // 3. We read the first page of four.
    // 4. We read the second page, before the last id of the first, which
    //    holds the two oldest rows.
    // 5. We narrow the trail to ada, which holds the five creates, and to an
    //    unknown name, which is empty.
    on_each_backend(|backend| {
        // 1. create acme and log ada in
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let password = create_acme(&server, &token);
        let (ada, _) = log_in(&server, "acme", "ada@example.com", &password);
        // 2. create five users
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

        // 3. read the first page
        let first = trail(&server, &ada, "acme", "?limit=4");
        assert_eq!(first.len(), 4, "{first:?} on {backend}");
        let ids: Vec<&str> = first
            .iter()
            .map(|r| r["id"].as_str().expect("an id"))
            .collect();
        // The text of a version 7 UUID sorts by time, so descending ids run
        // from newest to oldest.
        assert!(ids.windows(2).all(|w| w[0] > w[1]), "{ids:?} on {backend}");
        assert_eq!(first[0]["route"], "/api/v1/tenants/{tenant}/users");

        // 4. read the second page
        let second = trail(
            &server,
            &ada,
            "acme",
            &format!("?limit=4&before={}", ids[3]),
        );
        assert_eq!(second.len(), 2, "{second:?} on {backend}");
        assert!(second[0]["id"].as_str().expect("an id") < ids[3]);
        assert_eq!(second[1]["route"], "/api/v1/login");

        // 5. narrow to one user
        // The mixed case shows that the filter matches a name in any case.
        let narrowed = trail(&server, &ada, "acme", "?user=Ada@Example.com");
        assert_eq!(narrowed.len(), 5, "{narrowed:?} on {backend}");
        assert!(narrowed.iter().all(|r| r["user"] == "ada@example.com"));
        let none = trail(&server, &ada, "acme", "?user=nobody@example.com");
        assert!(none.is_empty(), "{none:?} on {backend}");
    });
}

/// A row of a user deleted since still names that user, and the user filter
/// still finds the deleted user.
#[test]
fn the_trail_names_a_deleted_user() {
    // The test has a user act once, deletes the user, and reads the trail
    // narrowed to that user.
    //
    // 1. We create `acme`, log ada in, and create bob.
    // 2. Bob logs in and out. The logout is the only row that names bob,
    //    because a login row names no user.
    // 3. Ada deletes bob, which keeps his row in the users table.
    // 4. We narrow the trail to bob, which must hold the logout with his name
    //    and tenant, because the trail joins the users table on the id alone.
    on_each_backend(|backend| {
        // 1. create acme, ada and bob
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
        // 2. bob logs in and out
        let (bob, _) = log_in(
            &server,
            "acme",
            "bob@example.com",
            created["password"].as_str().expect("a password"),
        );
        let response = call(&server, &bob, "POST", "/api/v1/logout", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        // 3. ada deletes bob
        let response = call(
            &server,
            &ada,
            "DELETE",
            "/api/v1/tenants/acme/users/bob@example.com",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);

        // 4. narrow the trail to bob
        let rows = trail(&server, &ada, "acme", "?user=bob@example.com");
        assert_eq!(rows.len(), 1, "{rows:?} on {backend}");
        assert_eq!(rows[0]["route"], "/api/v1/logout");
        assert_eq!(rows[0]["user"], "bob@example.com");
        assert_eq!(rows[0]["user_tenant"], "acme");
    });
}
