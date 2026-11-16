//! The superadmin's mint route on the router served in this process, on each
//! backend.

mod common;

use common::{InProcess, call, log_in, on_each_backend, serve_in_process, superadmin_token};
use serde_json::{Value, json};

/// With the superadmin's `token`, creates the tenant `acme` whose admin is
/// `ada@example.com`, and returns the admin's one-time password.
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

/// A session that the superadmin mints for a user works as the user's own in
/// `me` and `refresh`, and the mint response carries no `Set-Cookie` header.
/// A logout from the minted session also makes the token of the user's own
/// login fail.
#[test]
fn superadmin_mints_a_session_that_acts_as_the_user() {
    // The superadmin mints a session for a tenant's admin, and the test uses
    // it as the admin and then logs out from it.
    //
    // 1. We create `acme` and log its admin in on her own, for step 4.
    // 2. We mint a session for the admin, which must be a login body with a
    //    token type, the must-change flag and both tokens.
    // 3. We call `me` and `refresh` with the minted tokens.
    // 4. We log out from the minted session, which must also end the admin's
    //    own session. A logout moves the user's cutoff, the time before which
    //    every token of the user is refused (ADR-0025).
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);

        // 1. create the tenant and log the admin in
        let password = create_acme(&server, &token);
        let (own, _) = log_in(&server, "acme", "ada@example.com", &password);

        // 2. mint a session
        // The mixed case shows that the mint lowercases both names, as login
        // does.
        let response = call(
            &server,
            &token,
            "POST",
            "/api/v1/tenants/Acme/users/Ada@Example.com/session",
            None,
        );
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        // The mint sets no cookie, so the superadmin's own browser session
        // stays and the client decides what to do with the tokens (ADR-0030).
        assert_eq!(response.header_values("set-cookie"), [] as [&str; 0]);
        let minted: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(minted["token_type"], "Bearer");
        // The admin has not changed her generated password, and the mint
        // passes the flag through.
        assert_eq!(minted["password_must_change"], true);
        let access = minted["access_token"].as_str().expect("an access token");
        let refresh = minted["refresh_token"].as_str().expect("a refresh token");

        // 3. use the minted tokens
        let response = call(&server, access, "GET", "/api/v1/me", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let me: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(me["name"], "ada@example.com");
        assert_eq!(me["role"], "admin");
        let body = json!({ "refresh_token": refresh });
        // Refresh takes the refresh token from the body and reads no bearer,
        // and `call` always sends one.
        let response = call(&server, access, "POST", "/api/v1/refresh", Some(&body));
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        let refreshed: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert!(refreshed["access_token"].is_string());

        // 4. log out from the minted session
        let response = call(&server, access, "POST", "/api/v1/logout", None);
        assert_eq!(response.status, 200, "{} on {backend}", response.body);
        // Several requests lie between the admin's login and the logout, so
        // the cutoff falls after the issue time of `own`.
        assert_eq!(call(&server, &own, "GET", "/api/v1/me", None).status, 401);
        assert_eq!(call(&server, access, "GET", "/api/v1/me", None).status, 401);
    });
}

/// A mint for an unknown tenant, an unknown user, or a deleted user is 404,
/// and an admin minting is 403.
#[test]
fn mint_refuses_unknown_targets_and_lower_roles() {
    // The test prepares a tenant with a deleted user, then calls the mint as
    // the superadmin and as the admin.
    //
    // 1. We create `acme` and a user that we delete again, a target whose row
    //    stays but is no longer live.
    // 2. We mint for an unknown tenant, an unknown user and the deleted user.
    //    The mint answers 404 and not login's 401, because nothing is hidden
    //    from the superadmin.
    // 3. We have the admin mint for her own account. The mint refuses a lower
    //    role before any lookup, so the target plays no part in the 403.
    on_each_backend(|backend| {
        // 1. prepare the tenant and a deleted user
        let server = serve_in_process(backend);
        let token = superadmin_token(&server);
        let password = create_acme(&server, &token);
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

        // 2. mint for targets that are not live
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

        // 3. mint as the admin
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
