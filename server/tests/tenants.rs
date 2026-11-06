//! The superadmin's tenant routes on the router served in this process, on
//! each backend: a scenario, a property over generated names, and every role
//! against every route's minimum.

mod common;

use common::{
    ADMIN, InProcess, Response, SYSTEM_TENANT, on_each_backend, request, serve_in_process,
};
use proptest::prelude::*;
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

/// Whether `name` passes the tenant name rule.
fn valid_tenant(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c))
}

/// Whether `name` passes the user name rule.
fn valid_user(name: &str) -> bool {
    (3..=254).contains(&name.len())
        && !name.chars().any(char::is_whitespace)
        && !name.starts_with('$')
        && name.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && !domain.is_empty() && !domain.contains('@')
        })
}

/// A tenant name in mixed case. One in three breaks the rule: a character
/// outside it, the reserved prefix, no characters, or too many.
fn tenant_name() -> impl Strategy<Value = String> {
    let valid = "[a-zA-Z0-9._-]{1,64}";
    let invalid = prop_oneof![
        "[a-z]{1,3}[ !@/#:$\u{e9}][a-z]{0,3}",
        "\\$[a-z]{1,4}",
        Just(String::new()),
        Just("a".repeat(65)),
    ];
    prop_oneof![2 => valid.prop_map(String::from), 1 => invalid]
}

/// An admin's email address in mixed case, whose local part may hold `$`
/// anywhere, the first character too. One in three breaks the shape: no `@`,
/// a second `@`, whitespace, or too many characters, or it starts with the
/// reserved prefix.
fn admin_name() -> impl Strategy<Value = String> {
    let valid = "[a-zA-Z0-9._$]{1,12}@[a-zA-Z]{1,12}\\.[a-z]{2,4}";
    let invalid = prop_oneof![
        "\\$[a-z]{1,5}@[a-z]{1,5}\\.io",
        "[a-z]{1,10}",
        "[a-z]{1,5}@[a-z]{1,5}@[a-z]{1,5}",
        "[a-z]{1,5} [a-z]{1,5}@[a-z]{1,5}\\.io",
        Just(format!("{}@example.io", "a".repeat(250))),
    ];
    prop_oneof![2 => valid.prop_map(String::from), 1 => invalid]
}

proptest! {
    // 16 cases rather than proptest's 256: each case builds a server and
    // hashes Argon2id passwords, on every backend. No regression file, so a
    // failure leaves nothing untracked in the tree; the failing seed is in
    // the output.
    #![proptest_config(ProptestConfig {
        cases: 16,
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A create request succeeds exactly when the tenant name and the admin's
    /// name pass their rules, and the stored names are lowercase. Names that
    /// start with `$` are refused with 400.
    #[test]
    fn tenant_creation_follows_the_name_rules(tenant in tenant_name(), admin in admin_name()) {
        let (lower_tenant, lower_admin) = (tenant.to_lowercase(), admin.to_lowercase());
        let valid = valid_tenant(&lower_tenant) && valid_user(&lower_admin);
        for backend in common::backends() {
            let server = serve_in_process(backend);
            let token = superadmin_token(&server);
            let new = json!({ "name": tenant, "title": "T", "admin": admin });
            let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
            if !valid {
                prop_assert_eq!(response.status, 400, "{:?} {:?} on {}: {}", tenant, admin, backend, response.body);
                continue;
            }
            prop_assert_eq!(response.status, 201, "{:?} {:?} on {}: {}", tenant, admin, backend, response.body);
            let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
            prop_assert_eq!(created["name"].as_str(), Some(lower_tenant.as_str()));
            prop_assert_eq!(created["admin"].as_str(), Some(lower_admin.as_str()));
        }
    }
}
