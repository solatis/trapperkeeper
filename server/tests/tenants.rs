//! The superadmin's tenant routes on the router served in this process, on
//! each backend: a scenario, a property over generated names, and every role
//! against every route's minimum.

mod common;

use common::{
    ADMIN, InProcess, Response, SYSTEM_TENANT, on_each_backend, request, serve_in_process,
};
use serde_json::{Value, json};

/// Logs in to `tenant` as `name` with `password`, and returns the access
/// token and whether the password must change. Panics when the login fails.
fn log_in(server: &InProcess, tenant: &str, name: &str, password: &str) -> (String, bool) {
    let credentials = json!({ "tenant": tenant, "name": name, "password": password });
    let response = request(
        server.addr,
        "POST",
        "/api/v1/login",
        &[],
        Some(&credentials.to_string()),
    );
    assert_eq!(response.status, 200, "{}", response.body);
    let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
    (
        login["access_token"]
            .as_str()
            .expect("an access token")
            .to_owned(),
        login["password_must_change"].as_bool().expect("a flag"),
    )
}

/// The superadmin's access token.
fn superadmin_token(server: &InProcess) -> String {
    log_in(server, SYSTEM_TENANT, ADMIN, &server.admin_password).0
}

/// Sends `method` to `path` with `token` as the bearer, and `body` as JSON
/// when there is one.
fn call(
    server: &InProcess,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Response {
    let bearer = format!("Bearer {token}");
    let body = body.map(Value::to_string);
    request(
        server.addr,
        method,
        path,
        &[("Authorization", &bearer)],
        body.as_deref(),
    )
}

/// The superadmin creates a tenant, the new tenant shows in the list, and its
/// admin logs in with the returned password and must change it. The same
/// name again gets 409.
#[test]
fn superadmin_creates_a_tenant_whose_admin_logs_in() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let new = json!({ "name": "Acme", "title": " Acme Corp ", "admin": "Ada@Example.com" });

        let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
        assert_eq!(response.status, 201, "{} on {backend}", response.body);
        let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(created["name"], "acme");
        assert_eq!(created["title"], "Acme Corp");
        assert_eq!(created["admin"], "ada@example.com");
        let password = created["admin_password"].as_str().expect("a password");

        let response = call(&server, &token, "GET", "/api/v1/tenants", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(listed, json!([{ "name": "acme", "title": "Acme Corp" }]));

        let (admin_token, must_change) = log_in(&server, "acme", "ada@example.com", password);
        assert!(must_change);
        let response = call(&server, &admin_token, "GET", "/api/v1/me", None);
        let me: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(me["role"], "admin");

        let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
        assert_eq!(response.status, 409, "{} on {backend}", response.body);
    });
}
