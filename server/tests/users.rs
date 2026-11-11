//! The admin's user routes on the router served in this process, on each
//! backend: a scenario, the superadmin's direct access, the refusals, and the
//! tenant scope.

mod common;

use common::{InProcess, call, log_in, on_each_backend, serve_in_process, superadmin_token};
use serde_json::{Value, json};

/// Creates the tenant `name` with the admin `admin` as the superadmin, and
/// returns the admin's password.
fn create_tenant(server: &InProcess, token: &str, name: &str, admin: &str) -> String {
    let new = json!({ "name": name, "title": "T", "admin": admin });
    let response = call(server, token, "POST", "/api/v1/tenants", Some(&new));
    assert_eq!(response.status, 201, "{}", response.body);
    let created: Value = serde_json::from_str(&response.body).expect("a JSON body");
    created["admin_password"]
        .as_str()
        .expect("a password")
        .to_owned()
}

/// Creates the user `name` with `role` in `tenant` as `token`, and returns
/// the response body.
fn create_user(server: &InProcess, token: &str, tenant: &str, name: &str, role: &str) -> Value {
    let new = json!({ "name": name, "role": role });
    let path = format!("/api/v1/tenants/{tenant}/users");
    let response = call(server, token, "POST", &path, Some(&new));
    assert_eq!(response.status, 201, "{}", response.body);
    serde_json::from_str(&response.body).expect("a JSON body")
}

/// The status of `GET /me` with `token`.
fn me_status(server: &InProcess, token: &str) -> u16 {
    call(server, token, "GET", "/api/v1/me", None).status
}

/// A scenario: the superadmin creates a tenant; its admin creates an operator
/// and an observer, lists the three, changes the operator to admin, deletes
/// themself, and the remaining admin's delete of themself gets 409; the
/// observer deletes itself and its next call is 401; a reset password ends
/// the user's session, logs the user in and must change.
#[test]
fn admin_manages_the_users_of_its_tenant() {
    // 1. create the tenant and log in as its admin
    // 2. create an operator and an observer
    // 3. list the users
    // 4. promote the operator to admin
    // 5. the first admin deletes themself
    // 6. the remaining admin's self delete gets 409
    // 7. the observer deletes itself
    // 8. reset a user's password
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1. The admin logs in with the password the create returned.
        let password = create_tenant(&server, &token, "acme", "ada@example.com");
        let (ada, _) = log_in(&server, "acme", "ada@example.com", &password);

        // 2. Mixed case, so the response shows the name lowercased as it is
        //    stored.
        let ops = create_user(&server, &ada, "acme", "Ops@Example.com", "operator");
        assert_eq!(ops["name"], "ops@example.com");
        assert_eq!(ops["role"], "operator");
        let ops_password = ops["password"].as_str().expect("a password");
        let tenant_id = ops["tenant_id"].as_str().expect("a tenant id");
        let watcher = create_user(&server, &ada, "acme", "obs@example.com", "observer");
        let watcher_password = watcher["password"].as_str().expect("a password");

        // 3. By name, each with the tenant's id and the must-change flag of a
        //    generated password.
        let response = call(&server, &ada, "GET", "/api/v1/tenants/acme/users", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        let row = |name: &str, role: &str| json!({ "tenant_id": tenant_id, "name": name, "role": role, "password_must_change": true });
        assert_eq!(
            listed,
            json!([
                row("ada@example.com", "admin"),
                row("obs@example.com", "observer"),
                row("ops@example.com", "operator")
            ])
        );

        // 4. The operator becomes a second admin, which step 5 relies on.
        let change = json!({ "role": "admin" });
        let response = call(
            &server,
            &ada,
            "PUT",
            "/api/v1/tenants/acme/users/ops@example.com/role",
            Some(&change),
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let changed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(changed["role"], "admin");

        // 5. Allowed because the operator was promoted first, so the guard
        //    sees two admins. The deleted admin's next call is 401, because
        //    the user row is gone from every read.
        let response = call(
            &server,
            &ada,
            "DELETE",
            "/api/v1/tenants/acme/users/ada@example.com",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        assert_eq!(me_status(&server, &ada), 401);

        // 6. The promoted operator is now the last live admin.
        let (ops, _) = log_in(&server, "acme", "ops@example.com", ops_password);
        let response = call(
            &server,
            &ops,
            "DELETE",
            "/api/v1/tenants/acme/users/ops@example.com",
            None,
        );
        assert_eq!(response.status, 409, "{} on {backend}", response.body);

        // 7. The name is percent-encoded, so the test sees that the path
        //    segment is decoded before the lookup.
        let (watcher, _) = log_in(&server, "acme", "obs@example.com", watcher_password);
        let response = call(
            &server,
            &watcher,
            "DELETE",
            "/api/v1/tenants/acme/users/obs%40example.com",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        assert_eq!(me_status(&server, &watcher), 401);

        // 8. The reset ends the session held with the old password, and the
        //    new password logs in and must change.
        let eve = create_user(&server, &ops, "acme", "eve@example.com", "observer");
        let (eve_before, _) = log_in(
            &server,
            "acme",
            "eve@example.com",
            eve["password"].as_str().expect("a password"),
        );
        let response = call(
            &server,
            &ops,
            "POST",
            "/api/v1/tenants/acme/users/eve@example.com/password",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let reset: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(reset["name"], "eve@example.com");
        assert_eq!(me_status(&server, &eve_before), 401);
        let (_, must_change) = log_in(
            &server,
            "acme",
            "eve@example.com",
            reset["password"].as_str().expect("a password"),
        );
        assert!(must_change);
    });
}

/// The superadmin manages a tenant's users directly: creates one in a tenant
/// it is not a member of and lists them, and every user route on `$tk`
/// answers 404, so the superadmin cannot be listed or deleted.
#[test]
fn superadmin_manages_any_tenants_users() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        create_tenant(&server, &token, "acme", "ada@example.com");

        let created = create_user(&server, &token, "acme", "bob@example.com", "operator");
        assert_eq!(created["role"], "operator");
        let response = call(&server, &token, "GET", "/api/v1/tenants/acme/users", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(listed.as_array().map(Vec::len), Some(2));

        // The system tenant is answered as unknown, whoever asks. The bodies
        // name an ordinary role, because `superadmin` is refused with 400
        // before any tenant is looked up.
        let new = json!({ "name": "x@example.com", "role": "observer" });
        let change = json!({ "role": "observer" });
        let on_system: [(&str, &str, Option<&Value>); 5] = [
            ("GET", "/api/v1/tenants/$tk/users", None),
            ("POST", "/api/v1/tenants/$tk/users", Some(&new)),
            ("DELETE", "/api/v1/tenants/$tk/users/admin", None),
            ("PUT", "/api/v1/tenants/$tk/users/admin/role", Some(&change)),
            ("POST", "/api/v1/tenants/$tk/users/admin/password", None),
        ];
        for (method, path, body) in on_system {
            let response = call(&server, &token, method, path, body);
            assert_eq!(
                response.status, 404,
                "{method} {path} on {backend}: {}",
                response.body
            );
        }
        assert_eq!(me_status(&server, &token), 200);
    });
}

/// The refusals: a bad email, the superadmin role, a role the tenant does not
/// hold, a taken name, and an observer deleting another user, which is 403
/// whether or not the name exists.
#[test]
fn user_routes_refuse_bad_names_roles_and_other_users() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let password = create_tenant(&server, &token, "acme", "ada@example.com");
        let (ada, _) = log_in(&server, "acme", "ada@example.com", &password);

        let refused: [(&str, &str, Value, u16); 6] = [
            (
                "POST",
                "/api/v1/tenants/acme/users",
                json!({ "name": "no-at-sign", "role": "observer" }),
                400,
            ),
            (
                "POST",
                "/api/v1/tenants/acme/users",
                json!({ "name": "x@example.com", "role": "superadmin" }),
                400,
            ),
            (
                "POST",
                "/api/v1/tenants/acme/users",
                json!({ "name": "x@example.com", "role": "auditor" }),
                400,
            ),
            (
                "POST",
                "/api/v1/tenants/acme/users",
                json!({ "name": "Ada@Example.com", "role": "observer" }),
                409,
            ),
            (
                "PUT",
                "/api/v1/tenants/acme/users/ada@example.com/role",
                json!({ "role": "superadmin" }),
                400,
            ),
            (
                "PUT",
                "/api/v1/tenants/acme/users/ada@example.com/role",
                json!({ "role": "auditor" }),
                400,
            ),
        ];
        for (method, path, body, status) in &refused {
            let response = call(&server, &ada, method, path, Some(body));
            assert_eq!(
                response.status, *status,
                "{method} {path} {body} on {backend}: {}",
                response.body
            );
        }

        // A non-admin learns nothing about other names from the delete route.
        let obs = create_user(&server, &ada, "acme", "obs@example.com", "observer");
        let (obs, _) = log_in(
            &server,
            "acme",
            "obs@example.com",
            obs["password"].as_str().expect("a password"),
        );
        for name in ["ada@example.com", "nobody@example.com"] {
            let path = format!("/api/v1/tenants/acme/users/{name}");
            let response = call(&server, &obs, "DELETE", &path, None);
            assert_eq!(
                response.status, 403,
                "{name} on {backend}: {}",
                response.body
            );
        }
        // An admin's delete of a missing name is 404.
        let response = call(
            &server,
            &ada,
            "DELETE",
            "/api/v1/tenants/acme/users/nobody@example.com",
            None,
        );
        assert_eq!(response.status, 404, "{} on {backend}", response.body);
    });
}

/// Tenant isolation: an admin of tenant A gets 403 on tenant B's routes and
/// 404 on an unknown tenant (ADR-0003).
#[test]
fn user_routes_scope_to_the_tenant_in_the_path() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let password = create_tenant(&server, &token, "a", "ada@example.com");
        create_tenant(&server, &token, "b", "bea@example.com");
        let (ada, _) = log_in(&server, "a", "ada@example.com", &password);

        let new = json!({ "name": "x@example.com", "role": "observer" });
        let change = json!({ "role": "observer" });
        let routes: [(&str, &str, Option<&Value>); 5] = [
            ("GET", "users", None),
            ("POST", "users", Some(&new)),
            ("DELETE", "users/bea@example.com", None),
            ("PUT", "users/bea@example.com/role", Some(&change)),
            ("POST", "users/bea@example.com/password", None),
        ];
        for (tenant, status) in [("b", 403), ("nobody", 404)] {
            for (method, tail, body) in routes {
                let path = format!("/api/v1/tenants/{tenant}/{tail}");
                let response = call(&server, &ada, method, &path, body);
                assert_eq!(
                    response.status, status,
                    "{method} {path} on {backend}: {}",
                    response.body
                );
            }
        }
    });
}
