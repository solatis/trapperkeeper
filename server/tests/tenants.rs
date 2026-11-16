//! The superadmin's tenant routes on the router served in this process, on
//! each backend, as a scenario and a property over generated names. The file
//! also checks every role against the minimum of every route that checks
//! one, the user, session, audit and account routes included.

mod common;

use common::{call, log_in, on_each_backend, on_pool, serve_in_process, superadmin_token};
use proptest::prelude::*;
use serde_json::{Value, json};

/// The superadmin creates a tenant, the new tenant shows in the list, and its
/// admin logs in with the returned password and must change it. The same
/// name again gets 409.
#[test]
fn superadmin_creates_a_tenant_whose_admin_logs_in() {
    // The test follows one tenant through the list, its admin's first login
    // and a second create.
    //
    // 1. We create the tenant with mixed-case names and a padded title, which
    //    the response must show lowercased and trimmed, as they are stored.
    // 2. We list the tenants.
    // 3. We log the admin in, who must change the password and hold the role
    //    admin. The create gives the admin a generated password, as every new
    //    user gets.
    // 4. We create the same tenant again, which must get 409, because a name
    //    is unique among the tenants that are not deleted.
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1. create the tenant
        let new = json!({ "name": "Acme", "title": " Acme Corp ", "admin": "Ada@Example.com" });

        let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
        assert_eq!(response.status, 201, "{} on {backend}", response.body);
        let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(created["name"], "acme");
        assert_eq!(created["title"], "Acme Corp");
        assert_eq!(created["admin"], "ada@example.com");
        let password = created["admin_password"].as_str().expect("a password");

        // 2. list the tenants
        let response = call(&server, &token, "GET", "/api/v1/tenants", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        // The list leaves out the system tenant `$tk`.
        assert_eq!(listed, json!([{ "name": "acme", "title": "Acme Corp" }]));

        // 3. log in as the admin
        let (admin_token, must_change) = log_in(&server, "acme", "ada@example.com", password);
        assert!(must_change);
        let response = call(&server, &admin_token, "GET", "/api/v1/me", None);
        let me: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(me["role"], "admin");

        // 4. create it again
        let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
        assert_eq!(response.status, 409, "{} on {backend}", response.body);
    });
}

/// Whether `name` passes the rule of `names::tenant_name`: 1 to 64 of the
/// characters a-z, 0-9, `.`, `_` and `-`. The property below checks the route
/// against this model.
fn valid_tenant(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-_".contains(c))
}

/// Whether `name` passes the rule of `names::user_name`: an email address of 3
/// to 254 characters with one `@` and text on both sides, no whitespace and no
/// leading `$`.
fn valid_user(name: &str) -> bool {
    // Counts bytes, as `names::user_name` does.
    (3..=254).contains(&name.len())
        && !name.chars().any(char::is_whitespace)
        && !name.starts_with('$')
        && name.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && !domain.is_empty() && !domain.contains('@')
        })
}

/// A tenant name in mixed case. One in three breaks the rule: a character
/// outside it, a leading `$`, no characters, or too many.
fn tenant_name() -> impl Strategy<Value = String> {
    let valid = "[a-zA-Z0-9._-]{1,64}";
    let invalid = prop_oneof![
        // Each character in the bracket is outside the allowed set, the e
        // with an acute accent too, which lowercasing keeps.
        "[a-z]{1,3}[ !@/#:$\u{e9}][a-z]{0,3}",
        "\\$[a-z]{1,4}",
        Just(String::new()),
        Just("a".repeat(65)),
    ];
    prop_oneof![2 => valid.prop_map(String::from), 1 => invalid]
}

/// An admin's email address in mixed case. One in three breaks the rule with
/// no `@`, a second `@`, whitespace, too many characters or a leading `$`.
/// The valid alternative may also start with `$`, which the rule refuses too.
fn admin_name() -> impl Strategy<Value = String> {
    // The rule refuses only a leading `$`, so the local part may hold `$`.
    let valid = "[a-zA-Z0-9._$]{1,12}@[a-zA-Z]{1,12}\\.[a-z]{2,4}";
    let invalid = prop_oneof![
        "\\$[a-z]{1,5}@[a-z]{1,5}\\.io",
        "[a-z]{1,10}",
        "[a-z]{1,5}@[a-z]{1,5}@[a-z]{1,5}",
        "[a-z]{1,5} [a-z]{1,5}@[a-z]{1,5}\\.io",
        // 261 characters, past the 254 that the rule allows.
        Just(format!("{}@example.io", "a".repeat(250))),
    ];
    prop_oneof![2 => valid.prop_map(String::from), 1 => invalid]
}

proptest! {
    #![proptest_config(ProptestConfig {
        // The property runs 16 cases, because each serves a router on a fresh
        // database on every backend and hashes Argon2id passwords.
        cases: 16,
        // No regression file is left in the tree. The panic message shows the
        // minimal failing input.
        failure_persistence: None,
        .. ProptestConfig::default()
    })]

    /// A create request succeeds exactly when the tenant name and the admin's
    /// name pass their rules, and the stored names are lowercase. Names that
    /// start with `$` are refused with 400.
    #[test]
    fn tenant_creation_follows_the_name_rules(tenant in tenant_name(), admin in admin_name()) {
        // The model applies both rules to the lowercased names.
        let (lower_tenant, lower_admin) = (tenant.to_lowercase(), admin.to_lowercase());
        let valid = valid_tenant(&lower_tenant) && valid_user(&lower_admin);
        // `prop_assert_eq!` returns an error, which the closure of
        // `on_each_backend` cannot, so the property loops itself.
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

/// The rank of `role`, from 0 for the lowest, or `None` for a role that is
/// not on the ladder.
fn rank(role: &str) -> Option<u8> {
    // The order of the server's `Rank`.
    ["observer", "operator", "admin", "superadmin"]
        .iter()
        .position(|r| *r == role)
        .map(|p| u8::try_from(p).expect("four roles"))
}

/// Every route that checks a role, with its minimum, and the routes of the
/// caller's own account, `me`, the password change and logout, with `None`.
/// The user routes act on `acme`. The delete, role and password routes name a
/// user that does not exist, so the calls change nothing that the test reads
/// later. The password change goes without a body, so the server refuses it
/// after it accepts the caller.
const ROUTES: [(&str, &str, Option<&str>); 12] = [
    ("GET", "/api/v1/tenants", Some("superadmin")),
    ("POST", "/api/v1/tenants", Some("superadmin")),
    ("GET", "/api/v1/tenants/acme/users", Some("admin")),
    ("POST", "/api/v1/tenants/acme/users", Some("admin")),
    // A user may delete itself, so admin is the minimum for another's name.
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
    // The matrix test creates this user before it calls the routes, so the
    // superadmin's mint of a session finds a user. A mint writes no user row,
    // so it changes nothing that later calls read.
    (
        "POST",
        "/api/v1/tenants/acme/users/admin@example.com/session",
        Some("superadmin"),
    ),
    ("GET", "/api/v1/tenants/acme/audit", Some("admin")),
    ("GET", "/api/v1/me", None),
    ("POST", "/api/v1/password", None),
    // Logout comes last, because it revokes the token that the earlier calls
    // use.
    ("POST", "/api/v1/logout", None),
];

/// The request body that caller `role` sends as call `n`, `method` on `path`,
/// or `None`. Each create names a tenant or a user of its own, so no two
/// creates collide.
fn body_for(n: usize, role: &str, method: &str, path: &str) -> Option<Value> {
    // The last segment tells the routes with a body apart. `rsplit` yields at
    // least one segment, so the `?` never returns.
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

/// Every role, and a role that is not on the ladder, calls every route of
/// `ROUTES`. A call answers 403 when the role ranks below the route's minimum,
/// and any other call answers neither 401 nor 403, so the routes of the
/// caller's own account answer every role.
#[test]
fn every_role_meets_each_route_minimum() {
    // The test covers every pair of the five callers and the 12 routes,
    // because the set is small and fixed (ADR-0018).
    //
    // 1. We create `acme`, which seeds the roles admin, operator and observer,
    //    and read its id, which the create response does not return.
    // 2. We add the role `auditor`, which is not on the ladder, through sqlx,
    //    because no route creates a role.
    // 3. We create and log in one user per role through the user route.
    // 4. We call every route as every caller.
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1. create the tenant
        let new = json!({ "name": "acme", "title": "Acme", "admin": "ada@example.com" });
        let response = call(&server, &token, "POST", "/api/v1/tenants", Some(&new));
        assert_eq!(response.status, 201, "{} on {backend}", response.body);
        let tenant_id: String = on_pool!(server.database(), |pool| {
            sqlx::query_scalar("SELECT tenant_id FROM tenants WHERE tenant_name = 'acme'")
                .fetch_one(&pool)
                .await
                .expect("the tenant is read")
        });
        // 2. add a role off the ladder
        on_pool!(server.database(), |pool| {
            // A fixed version 7 UUID with a zero timestamp. The migrations
            // seed only ids ending in 0 and 1, and a tenant's roles get ids
            // from the clock.
            let insert = common::placeholders(
                backend,
                "INSERT INTO roles (role_id, tenant_id, role_name, role_title) \
                 VALUES ('00000000-0000-7000-8000-0000000000aa', $1, 'auditor', 'Auditor')",
            );
            // The statement is a literal, so no outside text reaches the SQL.
            sqlx::query(sqlx::AssertSqlSafe(insert))
                .bind(&tenant_id)
                .execute(&pool)
                .await
                .expect("the role is inserted");
        });

        // 3. create and log in one user per role
        let mut callers = vec![("superadmin", token.clone())];
        for role in ["admin", "operator", "observer", "auditor"] {
            let name = format!("{role}@example.com");
            let new = json!({ "name": name, "role": role });
            let response = call(
                &server,
                &token,
                "POST",
                "/api/v1/tenants/acme/users",
                Some(&new),
            );
            assert_eq!(response.status, 201, "{} on {backend}", response.body);
            let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
            let password = created["password"].as_str().expect("a password");
            // The must-change flag restricts no route.
            callers.push((role, log_in(&server, "acme", &name, password).0));
        }

        // 4. call every route as every caller
        for (role, token) in &callers {
            for (n, (method, path, minimum)) in ROUTES.iter().enumerate() {
                let body = body_for(n, role, method, path);
                let response = call(&server, token, method, path, body.as_ref());
                // The off-ladder role has no rank, so it counts as below
                // every minimum. Every minimum in `ROUTES` has a rank.
                let below = minimum.is_some_and(|m| rank(role).is_none_or(|r| Some(r) < rank(m)));
                let context = format!("{role} on {method} {path} on {backend}: {}", response.body);
                if below {
                    assert_eq!(response.status, 403, "{context}");
                    // Every route refuses a role through `Refusal::Forbidden`.
                    assert_eq!(response.body, "forbidden", "{context}");
                } else {
                    // A permitted call may still be refused on its content, as
                    // a delete of an unknown name is 404.
                    assert!(
                        response.status != 401 && response.status != 403,
                        "{context}"
                    );
                }
            }
        }
    });
}
