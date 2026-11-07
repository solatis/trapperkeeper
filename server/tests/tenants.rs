//! The superadmin's tenant routes on the router served in this process, on
//! each backend: a scenario, a property over generated names, and every role
//! against every route's minimum.

mod common;

use common::{
    block_on, call, log_in, on_each_backend, on_pool, serve_in_process, superadmin_token,
};
use proptest::prelude::*;
use serde_json::{Value, json};
use trapperkeeper_server::{NewAccount, create_account};

/// The superadmin creates a tenant, the new tenant shows in the list, and its
/// admin logs in with the returned password and must change it. The same
/// name again gets 409.
#[test]
fn superadmin_creates_a_tenant_whose_admin_logs_in() {
    // 1. create a tenant as the superadmin
    // 2. list the tenants
    // 3. log in as the new admin and read me
    // 4. create the same tenant again
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1. Mixed case and a padded title, so the response shows the names
        //    lowercased and the title trimmed as they are stored.
        let new = json!({ "name": "Acme", "title": " Acme Corp ", "admin": "Ada@Example.com" });

        let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
        assert_eq!(response.status, 201, "{} on {backend}", response.body);
        let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(created["name"], "acme");
        assert_eq!(created["title"], "Acme Corp");
        assert_eq!(created["admin"], "ada@example.com");
        let password = created["admin_password"].as_str().expect("a password");

        // 2. The list leaves `$tk` out, so the new tenant is the only entry.
        let response = call(&server, &token, "GET", "/api/v1/tenants", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(listed, json!([{ "name": "acme", "title": "Acme Corp" }]));

        // 3. The returned password logs the admin in and must be changed, and
        //    the admin holds the admin role the create seeded.
        let (admin_token, must_change) = log_in(&server, "acme", "ada@example.com", password);
        assert!(must_change);
        let response = call(&server, &admin_token, "GET", "/api/v1/me", None);
        let me: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(me["role"], "admin");

        // 4. The name is unique among live tenants.
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
        // The model: the rules apply to the lowercased names, and a request
        // passes only when both names pass.
        let (lower_tenant, lower_admin) = (tenant.to_lowercase(), admin.to_lowercase());
        let valid = valid_tenant(&lower_tenant) && valid_user(&lower_admin);
        for backend in common::backends() {
            let server = serve_in_process(backend);
            let token = superadmin_token(&server);
            let new = json!({ "name": tenant, "title": "T", "admin": admin });
            let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
            // A refused request writes nothing, so only the status is
            // checked.
            if !valid {
                prop_assert_eq!(response.status, 400, "{:?} {:?} on {}: {}", tenant, admin, backend, response.body);
                continue;
            }
            prop_assert_eq!(response.status, 201, "{:?} {:?} on {}: {}", tenant, admin, backend, response.body);
            // The response echoes the stored names.
            let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
            prop_assert_eq!(created["name"].as_str(), Some(lower_tenant.as_str()));
            prop_assert_eq!(created["admin"].as_str(), Some(lower_admin.as_str()));
        }
    }
}

/// The rank of each role on the ladder, lowest first, and `None` for a role
/// that is not on it.
fn rank(role: &str) -> Option<u8> {
    ["observer", "operator", "admin", "superadmin"]
        .iter()
        .position(|r| *r == role)
        .map(|p| u8::try_from(p).expect("four roles"))
}

/// Every route that checks a role, with its minimum, and the routes of the
/// caller's own account, with `None`. The user routes act on the matrix's
/// tenant `acme`, and the three that name a user name one that does not
/// exist, so the matrix changes nothing it later reads. The delete route takes
/// any caller and allows the target itself; its minimum for another user's
/// name is admin. Logout comes last, because it revokes the token every route
/// before it uses.
const ROUTES: [(&str, &str, Option<&str>); 11] = [
    ("GET", "/api/v1/tenants", Some("superadmin")),
    ("POST", "/api/v1/tenants", Some("superadmin")),
    ("GET", "/api/v1/tenants/acme/users", Some("admin")),
    ("POST", "/api/v1/tenants/acme/users", Some("admin")),
    (
        "DELETE",
        "/api/v1/tenants/acme/users/nobody@example.com",
        Some("admin"),
    ),
    (
        "PUT",
        "/api/v1/tenants/acme/users/nobody@example.com/role",
        Some("admin"),
    ),
    (
        "POST",
        "/api/v1/tenants/acme/users/nobody@example.com/password",
        Some("admin"),
    ),
    // The target exists in the matrix's tenant, so the superadmin's call is 200.
    (
        "POST",
        "/api/v1/tenants/acme/users/admin@example.com/session",
        Some("superadmin"),
    ),
    ("GET", "/api/v1/tenants/acme/audit", Some("admin")),
    ("GET", "/api/v1/me", None),
    ("POST", "/api/v1/logout", None),
];

/// The body route `n`, `method` on `path`, takes from caller `role`, or
/// `None`. Each create names a tenant or a user of its own, so a permitted
/// create succeeds.
fn body_for(n: usize, role: &str, method: &str, path: &str) -> Option<Value> {
    match (method, path.rsplit('/').next()?) {
        ("POST", "tenants") => {
            Some(json!({ "name": format!("t{n}-{role}"), "title": "T", "admin": "a@b.io" }))
        }
        ("POST", "users") => {
            Some(json!({ "name": format!("u{n}-{role}@example.com"), "role": "observer" }))
        }
        ("PUT", "role") => Some(json!({ "role": "observer" })),
        _ => None,
    }
}

/// Every role, plus a role that is not on the ladder, against every route:
/// 403 exactly when the role ranks below the route's minimum, neither 401 nor
/// 403 otherwise, and the routes of the caller's own account answer every
/// role.
#[test]
fn every_role_meets_each_route_minimum() {
    // The roles and routes are a small fixed set, so the test covers every
    // pair instead of sampling them (doc/adr/0018).
    //
    // 1. create a tenant, which seeds admin, operator and observer
    // 2. add a role that is not on the ladder
    // 3. create and log in one user per role
    // 4. call every route as every caller
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1.
        let new = json!({ "name": "acme", "title": "Acme", "admin": "ada@example.com" });
        let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
        assert_eq!(response.status, 201, "{} on {backend}", response.body);
        let tenant_id: String = on_pool!(server.database(), |pool| {
            sqlx::query_scalar("SELECT tenant_id FROM tenants WHERE tenant_name = 'acme'")
                .fetch_one(&pool)
                .await
                .expect("the tenant is read")
        });
        // 2. Inserted through sqlx, because no route creates roles.
        on_pool!(server.database(), |pool| {
            let insert = common::placeholders(
                backend,
                "INSERT INTO roles (role_id, tenant_id, role_name, role_title) \
                 VALUES ('00000000-0000-7000-8000-0000000000aa', $1, 'auditor', 'Auditor')",
            );
            sqlx::query(sqlx::AssertSqlSafe(insert))
                .bind(&tenant_id)
                .execute(&pool)
                .await
                .expect("the role is inserted");
        });

        // 3. Through the library, because no route creates a user of a
        //    given role yet.
        let mut callers = vec![("superadmin", token)];
        for role in ["admin", "operator", "observer", "auditor"] {
            let name = format!("{role}@example.com");
            let password = block_on(async {
                let mut tx = server.store.begin().await.expect("a transaction");
                let password = create_account(
                    &mut tx,
                    NewAccount {
                        tenant_id: &tenant_id,
                        name: &name,
                        role_name: role,
                    },
                )
                .await
                .expect("the user is created");
                tx.commit().await.expect("the user is committed");
                password
            });
            callers.push((role, log_in(&server, "acme", &name, &password).0));
        }

        // 4. A permitted call answers neither 401 nor 403, because it can
        //    still be refused on its content, as a delete of a name no user
        //    has is 404; a refused one answers 403 with the body `forbidden`.
        for (role, token) in &callers {
            for (n, (method, path, minimum)) in ROUTES.iter().enumerate() {
                let body = body_for(n, role, method, path);
                let response = call(&server, token, method, path, body.as_ref());
                // The off-ladder role has no rank, so it counts as below
                // every minimum.
                let below = minimum.is_some_and(|m| rank(role).is_none_or(|r| Some(r) < rank(m)));
                let context = format!("{role} on {method} {path} on {backend}: {}", response.body);
                if below {
                    assert_eq!(response.status, 403, "{context}");
                    assert_eq!(response.body, "forbidden", "{context}");
                } else {
                    assert!(
                        response.status != 401 && response.status != 403,
                        "{context}"
                    );
                }
            }
        }
    });
}
