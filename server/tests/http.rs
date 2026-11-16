//! Tests of the two probes, the API document and an unknown route on the
//! router served in this process. Each test runs on SQLite always, and on
//! Postgres and MariaDB when the environment sets their admin URLs.

mod common;

use common::{get, on_each_backend, serve_in_process};

#[test]
fn healthz_is_ok() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        assert_eq!(get(server.addr, "/healthz"), (200, "ok".to_owned()));
    });
}

/// Readiness is `ok` when the store answers.
#[test]
fn readyz_is_ok() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        assert_eq!(get(server.addr, "/readyz"), (200, "ok".to_owned()));
    });
}

/// The generated document lists the two probes, the account routes and
/// `/api/v1/tenants`, and not `/openapi.json`, the route that serves it. It
/// defines the bearer scheme, and `/api/v1/me` requires that scheme.
#[test]
fn openapi_describes_the_routes() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        let (status, body) = get(server.addr, "/openapi.json");
        assert_eq!(status, 200);
        let document: serde_json::Value =
            serde_json::from_str(&body).expect("the document is JSON");
        let paths = document["paths"].as_object().expect("a paths object");
        assert!(paths.contains_key("/healthz"));
        assert!(paths.contains_key("/readyz"));
        assert!(paths.contains_key("/api/v1/login"));
        assert!(paths.contains_key("/api/v1/refresh"));
        assert!(paths.contains_key("/api/v1/logout"));
        assert!(paths.contains_key("/api/v1/password"));
        assert!(paths.contains_key("/api/v1/me"));
        assert!(paths.contains_key("/api/v1/tenants"));
        assert!(!paths.contains_key("/openapi.json"));
        // The key is the name that routes cite in `security`, and the value is
        // the HTTP scheme registered under it.
        assert_eq!(
            document["components"]["securitySchemes"]["bearer"]["scheme"],
            "bearer"
        );
        // A security requirement maps a scheme to the scopes it needs, and
        // the bearer scheme needs none.
        assert_eq!(
            document["paths"]["/api/v1/me"]["get"]["security"][0]["bearer"],
            serde_json::json!([])
        );
    });
}

#[test]
fn unknown_route_is_404() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        assert_eq!(get(server.addr, "/nope").0, 404);
    });
}
