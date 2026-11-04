//! Runs the binary on each backend the environment names, with piped stdout,
//! and checks the shape of its log lines.

mod common;

use std::net::SocketAddr;

use common::{get, lines_until, on_each_backend, spawn_server};

/// Under a pipe every line is a JSON object with a timestamp, a level and its
/// fields, and the first lifecycle line names the address the server bound.
#[test]
fn piped_output_is_json_lines() {
    on_each_backend(|backend| {
        let server = spawn_server(backend, &[]);
        for line in &server.startup {
            assert!(line["timestamp"].is_string(), "{line}");
            assert!(line["level"].is_string(), "{line}");
            assert!(line["fields"].is_object(), "{line}");
        }
        let listening = server.startup.last().expect("the listening line");
        assert_eq!(listening["level"], "INFO");
        let address: SocketAddr = listening["fields"]["address"]
            .as_str()
            .expect("an address field")
            .parse()
            .expect("a socket address");
        // serve was started on port 0, so any other port is the one the
        // system assigned.
        assert_ne!(address.port(), 0);
    });
}

/// A request is one info line with the status and the latency, inside a span
/// that carries the method and the URI.
#[test]
fn request_is_logged() {
    on_each_backend(|backend| {
        let server = spawn_server(backend, &[]);
        assert_eq!(get(server.addr, "/healthz").0, 200);
        let lines = lines_until(&server.lines, "finished processing request");
        let request = lines.last().expect("the request line");
        assert_eq!(request["level"], "INFO");
        assert_eq!(request["fields"]["status"], 200);
        assert!(request["fields"]["latency"].is_string(), "{request}");
        assert_eq!(request["span"]["method"], "GET");
        assert_eq!(request["span"]["uri"], "/healthz");
    });
}
