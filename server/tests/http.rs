//! Probes the HTTP endpoints on each backend the environment names, on the
//! router served in this process.

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

/// The generated document describes the two probes and the management routes
/// under `/api/v1`, and not the route that serves the document.
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
        assert!(!paths.contains_key("/openapi.json"));
    });
}

#[test]
fn unknown_route_is_404() {
    on_each_backend(|backend| {
        let server = serve_in_process(backend);
        assert_eq!(get(server.addr, "/nope").0, 404);
    });
}
