//! Runs the binary on each backend the environment names, with piped stdout,
//! and checks the shape of its log lines.
//! Only these tests pipe stdout, and they drain it on a thread, because an
//! undrained pipe blocks the server once it is full.

mod common;

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Instant;

use common::{Backend, Server, TIMEOUT, get, on_each_backend, spawn_server};
use serde_json::Value;

/// Starts the server with piped stdout and a thread that forwards each line
/// of it, so a test can wait for a line with a deadline instead of blocking
/// on a read that may never return.
fn spawn_logged(backend: Backend) -> (Server, Receiver<String>) {
    let mut server = spawn_server(backend, &[], Stdio::piped());
    let stdout = server.child.stdout.take().expect("stdout is piped");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            // The receiver is gone once the test ends; the pipe closes when
            // the server is killed, which ends the loop.
            if sender.send(line.expect("a line of stdout")).is_err() {
                break;
            }
        }
    });
    (server, receiver)
}

/// Reads lines until one whose message is `message`, each parsed as JSON, and
/// returns all of them. Panics when `TIMEOUT` passes first.
fn lines_until(receiver: &Receiver<String>, message: &str) -> Vec<Value> {
    let deadline = Instant::now() + TIMEOUT;
    let mut lines = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let line = receiver
            .recv_timeout(remaining)
            .unwrap_or_else(|_| panic!("no line with message {message:?} within {TIMEOUT:?}"));
        let value: Value = serde_json::from_str(&line).expect("one JSON object per line");
        let found = value["fields"]["message"] == message;
        lines.push(value);
        if found {
            return lines;
        }
    }
}

/// Under a pipe every line is a JSON object with a timestamp, a level and its
/// fields, and the first lifecycle line names the bound address.
#[test]
fn piped_output_is_json_lines() {
    on_each_backend(|backend| {
        let (server, receiver) = spawn_logged(backend);
        let lines = lines_until(&receiver, "listening");
        for line in &lines {
            assert!(line["timestamp"].is_string(), "{line}");
            assert!(line["level"].is_string(), "{line}");
            assert!(line["fields"].is_object(), "{line}");
        }
        let listening = lines.last().expect("the listening line");
        assert_eq!(listening["level"], "INFO");
        assert_eq!(listening["fields"]["address"], server.addr.to_string());
    });
}

/// A request is one info line with the status and the latency, inside a span
/// that carries the method and the URI.
#[test]
fn request_is_logged() {
    on_each_backend(|backend| {
        let (server, receiver) = spawn_logged(backend);
        lines_until(&receiver, "listening");
        assert_eq!(get(server.addr, "/healthz").0, 200);
        let lines = lines_until(&receiver, "finished processing request");
        let request = lines.last().expect("the request line");
        assert_eq!(request["level"], "INFO");
        assert_eq!(request["fields"]["status"], 200);
        assert!(request["fields"]["latency"].is_string(), "{request}");
        assert_eq!(request["span"]["method"], "GET");
        assert_eq!(request["span"]["uri"], "/healthz");
    });
}
