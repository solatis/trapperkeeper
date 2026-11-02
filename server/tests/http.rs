//! Starts the binary and probes its HTTP endpoints.

mod common;

use std::process::Stdio;

use common::{get, spawn_server};

#[test]
fn healthz_is_ok() {
    let server = spawn_server(&[], Stdio::null());
    assert_eq!(get(server.addr, "/healthz"), (200, "ok".to_owned()));
}

/// Readiness is `ok` when the store answers.
#[test]
fn readyz_is_ok() {
    let server = spawn_server(&[], Stdio::null());
    assert_eq!(get(server.addr, "/readyz"), (200, "ok".to_owned()));
}

/// The generated document describes the two probes and not the route that
/// serves the document.
#[test]
fn openapi_describes_the_routes() {
    let server = spawn_server(&[], Stdio::null());
    let (status, body) = get(server.addr, "/openapi.json");
    assert_eq!(status, 200);
    let document: serde_json::Value = serde_json::from_str(&body).expect("the document is JSON");
    let paths = document["paths"].as_object().expect("a paths object");
    assert!(paths.contains_key("/healthz"));
    assert!(paths.contains_key("/readyz"));
    assert!(!paths.contains_key("/openapi.json"));
}

#[test]
fn unknown_route_is_404() {
    let server = spawn_server(&[], Stdio::null());
    assert_eq!(get(server.addr, "/nope").0, 404);
}

/// SIGTERM stops the server and it exits zero. Unix only: Windows has no
/// SIGTERM, and `kill` as a child process avoids a libc dependency.
#[cfg(unix)]
#[test]
fn sigterm_stops_the_server() {
    use std::process::Command;
    use std::thread;
    use std::time::{Duration, Instant};

    use common::TIMEOUT;

    let mut server = spawn_server(&[], Stdio::null());
    let sent = Command::new("kill")
        .arg("-TERM")
        .arg(server.child.id().to_string())
        .status()
        .expect("kill runs");
    assert!(sent.success());

    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = server.child.try_wait().expect("the child can be polled") {
            assert!(status.success(), "the server exited with {status}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the server did not stop within {TIMEOUT:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}
