//! Starts the binary on each backend the environment names and stops it with
//! a signal. Unix only: Windows has no SIGTERM.

#![cfg(unix)]

mod common;

use std::process::Stdio;

use common::{on_each_backend, spawn_server};

/// SIGTERM stops the server and it exits zero. `kill` as a child process
/// avoids a libc dependency.
#[test]
fn sigterm_stops_the_server() {
    on_each_backend(|backend| {
        use std::process::Command;
        use std::thread;
        use std::time::{Duration, Instant};

        use common::TIMEOUT;

        let mut server = spawn_server(backend, &[], Stdio::null());
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
    });
}
