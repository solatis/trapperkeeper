//! The admin's user routes on the router served in this process, on each
//! backend: a scenario, the superadmin's access to a tenant's users, the
//! refusals, and the tenant scope.

mod common;

use common::{InProcess, call, log_in, on_each_backend, serve_in_process, superadmin_token};
use serde_json::{Value, json};

/// With the superadmin's `token`, creates the tenant `name` whose first admin
/// is `admin`, and returns that admin's one-time password. Panics unless the
/// route answers 201.
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
/// the response body, which holds the one-time password. Panics unless the
/// route answers 201.
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

/// An admin creates, lists, changes the role of, deletes and resets the
/// password of the users of its own tenant. The last admin who is not deleted
/// cannot delete itself, and the token of a deleted user or from before a
/// password reset gets 401.
#[test]
fn admin_manages_the_users_of_its_tenant() {
    // The test runs one tenant through the user routes, and each step uses the
    // state that the step before left.
    //
    // 1. We create `acme` and log its admin in with the password that the
    //    create returned.
    // 2. We create an operator and an observer and keep their passwords.
    // 3. We list the users.
    // 4. We make the operator a second admin, so that the first admin may
    //    delete itself in step 5.
    // 5. The first admin deletes itself.
    // 6. The promoted operator, now the last live admin, may not delete
    //    itself.
    // 7. The observer deletes itself.
    // 8. We create a user and reset its password, which ends its session.
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1. create the tenant and log in
        let password = create_tenant(&server, &token, "acme", "ada@example.com");
        let (ada, _) = log_in(&server, "acme", "ada@example.com", &password);

        // 2. create an operator and an observer
        // The mixed case shows that the server stores the name lowercased.
        let ops_created = create_user(&server, &ada, "acme", "Ops@Example.com", "operator");
        assert_eq!(ops_created["name"], "ops@example.com");
        assert_eq!(ops_created["role"], "operator");
        let ops_password = ops_created["password"].as_str().expect("a password");
        let tenant_id = ops_created["tenant_id"].as_str().expect("a tenant id");
        let observer_created = create_user(&server, &ada, "acme", "obs@example.com", "observer");
        let observer_password = observer_created["password"].as_str().expect("a password");

        // 3. list the users
        let response = call(&server, &ada, "GET", "/api/v1/tenants/acme/users", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let listed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        // Every user still holds its generated password, and a login does not
        // clear the flag.
        let row = |name: &str, role: &str| json!({ "tenant_id": tenant_id, "name": name, "role": role, "password_must_change": true });
        // The route lists the users by name.
        assert_eq!(
            listed,
            json!([
                row("ada@example.com", "admin"),
                row("obs@example.com", "observer"),
                row("ops@example.com", "operator")
            ])
        );

        // 4. promote the operator
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

        // 5. the first admin deletes itself
        let response = call(
            &server,
            &ada,
            "DELETE",
            "/api/v1/tenants/acme/users/ada@example.com",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        // Every request reads the caller's user and refuses a deleted one.
        assert_eq!(me_status(&server, &ada), 401);

        // 6. the last admin may not delete itself
        let (ops, _) = log_in(&server, "acme", "ops@example.com", ops_password);
        let response = call(
            &server,
            &ops,
            "DELETE",
            "/api/v1/tenants/acme/users/ops@example.com",
            None,
        );
        assert_eq!(response.status, 409, "{} on {backend}", response.body);

        // 7. the observer deletes itself
        let (observer, _) = log_in(&server, "acme", "obs@example.com", observer_password);
        let response = call(
            &server,
            &observer,
            "DELETE",
            // The 200 shows that the server decodes the percent-encoded name,
            // which undecoded would name another user and get 403.
            "/api/v1/tenants/acme/users/obs%40example.com",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        assert_eq!(me_status(&server, &observer), 401);

        // 8. reset a user's password
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
        // The reset hashes a new password and then moves eve's cutoff, the
        // time before which her tokens are refused, past this token
        // (ADR-0025).
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

/// The superadmin creates and lists the users of a tenant it does not belong
/// to, without a session minted as one of its users. Every user route on the
/// system tenant `$tk` answers 404 to the superadmin, as for an unknown
/// tenant, because `$tk` is no customer (ADR-0028). So no user route lists or
/// changes the superadmin.
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
        // The first admin `ada` and `bob`.
        assert_eq!(listed.as_array().map(Vec::len), Some(2));

        // The bodies name an ordinary role, because `superadmin` is refused
        // with 400 before any tenant is looked up.
        let new = json!({ "name": "x@example.com", "role": "observer" });
        let change = json!({ "role": "observer" });
        let on_system: [(&str, &str, Option<&Value>); 5] = [
            ("GET", "/api/v1/tenants/$tk/users", None),
            ("POST", "/api/v1/tenants/$tk/users", Some(&new)),
            // `admin` is the superadmin's own name in `$tk`.
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
        // The superadmin is still live after the delete aimed at it.
        assert_eq!(me_status(&server, &token), 200);
    });
}

/// The user routes refuse a bad email, the role `superadmin`, a role the
/// tenant does not hold and a taken name. An observer who deletes another user
/// gets 403 whether or not the name exists, while an admin gets 404 for a
/// missing name.
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
                // A created tenant holds admin, operator and observer alone.
                json!({ "name": "x@example.com", "role": "auditor" }),
                400,
            ),
            (
                "POST",
                "/api/v1/tenants/acme/users",
                // Lowercased, the name is the admin's.
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
                // 400 and not 409 for the last admin, because the role is
                // checked before the admins are counted.
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
        let obs_created = create_user(&server, &ada, "acme", "obs@example.com", "observer");
        let (obs, _) = log_in(
            &server,
            "acme",
            "obs@example.com",
            obs_created["password"].as_str().expect("a password"),
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

/// An admin of tenant `a` gets 403 on every user route of tenant `b`, and 404
/// on a tenant that does not exist. The routes check the tenant in the path
/// before the user, so the 404 is for the missing tenant.
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
            // `bea@example.com` is the admin of `b`.
            ("DELETE", "users/bea@example.com", None),
            ("PUT", "users/bea@example.com/role", Some(&change)),
            // A reset generates the new password, so it takes no body.
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
