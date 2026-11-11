//! The superadmin's mint route on the router served in this process, on each
//! backend: a minted session acting as the user, and the refusals.

mod common;

use common::{InProcess, call, log_in, on_each_backend, serve_in_process, superadmin_token};
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

/// The superadmin creates a tenant, mints a session for its admin, and the
/// tokens work as the admin: me shows the admin, refresh yields a new access
/// token, no Set-Cookie header is in the mint response, and logout from the
/// minted session makes the admin's own login's token fail too.
#[test]
fn superadmin_mints_a_session_that_acts_as_the_user() {
    // 1. create the tenant and log the admin in on its own
    // 2. mint a session for the admin
    // 3. use the minted access token and refresh token as the admin
    // 4. log out from the minted session
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1. The admin's own session is what step 4's logout must end too.
        let password = create_acme(&server, &token);
        let (own, _) = log_in(&server, "acme", "ada@example.com", &password);

        // 2. Mixed case in the path, so the test sees both names lowercased
        //    before the lookup.
        let response = call(
            &server,
            &token,
            "POST",
            "/api/v1/tenants/Acme/users/Ada@Example.com/session",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        assert_eq!(response.header_values("set-cookie"), [] as [&str; 0]);
        let minted: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(minted["token_type"], "Bearer");
        assert_eq!(minted["password_must_change"], true);
        let access = minted["access_token"].as_str().expect("an access token");
        let refresh = minted["refresh_token"].as_str().expect("a refresh token");

        // 3. The tokens are the admin's own kind: every route reads them as
        //    the admin.
        let response = call(&server, access, "GET", "/api/v1/me", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let me: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(me["name"], "ada@example.com");
        assert_eq!(me["role"], "admin");
        let body = json!({ "refresh_token": refresh });
        let response = call(&server, access, "POST", "/api/v1/refresh", Some(&body));
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let refreshed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert!(refreshed["access_token"].is_string());

        // 4. The cutoff is per user, so the admin's own session ends too
        //    (ADR-0025).
        let response = call(&server, access, "POST", "/api/v1/logout", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        assert_eq!(call(&server, &own, "GET", "/api/v1/me", None).status, 401);
        assert_eq!(call(&server, access, "GET", "/api/v1/me", None).status, 401);
    });
}

/// A mint for an unknown tenant, an unknown user, or a deleted user is 404,
/// and an admin minting is 403.
#[test]
fn mint_refuses_unknown_targets_and_lower_roles() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let password = create_acme(&server, &token);

        // A user that was deleted since: created and deleted through the
        // user routes.
        let new = json!({ "name": "bob@example.com", "role": "observer" });
        let response = call(
            &server,
            &token,
            "POST",
            "/api/v1/tenants/acme/users",
            Some(&new),
        );
        assert_eq!(response.status, 201, "{} on {backend}", response.body);
        let response = call(
            &server,
            &token,
            "DELETE",
            "/api/v1/tenants/acme/users/bob@example.com",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);

        for path in [
            "/api/v1/tenants/nobody/users/ada@example.com/session",
            "/api/v1/tenants/acme/users/nobody@example.com/session",
            "/api/v1/tenants/acme/users/bob@example.com/session",
        ] {
            let response = call(&server, &token, "POST", path, None);
            assert_eq!(
                response.status, 404,
                "{path} on {backend}: {}",
                response.body
            );
        }

        let (ada, _) = log_in(&server, "acme", "ada@example.com", &password);
        let response = call(
            &server,
            &ada,
            "POST",
            "/api/v1/tenants/acme/users/ada@example.com/session",
            None,
        );
        assert_eq!(response.status, 403, "{} on {backend}", response.body);
    });
}
