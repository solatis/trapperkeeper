//! Logs in on the router served in this process, on each backend the
//! environment names, and uses what login hands out.

mod common;

use common::{ADMIN, on_each_backend, request, serve_in_process};
use serde_json::{Value, json};

/// A login with the admin's name, in any case, and its password returns a
/// bearer access token that lives an hour and says the password must change.
/// A wrong password and an unknown name get the same refusal.
#[test]
fn login_returns_a_token() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let credentials =
            json!({ "name": ADMIN.to_uppercase(), "password": server.admin_password });
        let response = request(
            server.addr,
            "POST",
            "/api/v1/login",
            &[],
            Some(&credentials.to_string()),
        );
        assert_eq!(response.status, 200, "{}", response.body);
        let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
        assert_eq!(login["token_type"], "Bearer");
        assert_eq!(login["expires_in"], 3600);
        assert_eq!(login["password_must_change"], true);
        let token = login["access_token"].as_str().expect("an access token");
        // Three base64url parts, as a JWT is.
        assert_eq!(token.split('.').count(), 3);

        for wrong in [
            json!({ "name": ADMIN, "password": "not the password" }),
            json!({ "name": "nobody", "password": server.admin_password }),
        ] {
            let response = request(
                server.addr,
                "POST",
                "/api/v1/login",
                &[],
                Some(&wrong.to_string()),
            );
            assert_eq!(response.status, 401);
            assert_eq!(response.body, "unauthorized");
        }
    });
}
