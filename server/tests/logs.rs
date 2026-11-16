//! The shape of the JSON log lines that the router served in this process
//! writes for a request. The file holds a single test, because that test
//! installs the process's global subscriber, which a process accepts once,
//! and each test file runs as a binary of its own.

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use common::{Backend, get, serve_in_process};
use serde_json::Value;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use trapperkeeper_server::json_subscriber;

/// A writer onto a buffer that the test reads back. Every clone writes into
/// the same bytes.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Buffer {
    /// Every line written so far, each parsed as a JSON value. Panics when
    /// the bytes are not UTF-8 or a line is not JSON.
    fn lines(&self) -> Vec<Value> {
        let bytes = self.0.lock().expect("the buffer is not poisoned");
        String::from_utf8(bytes.clone())
            .expect("UTF-8 log lines")
            .lines()
            .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
            .collect()
    }
}

impl io::Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("the buffer is not poisoned")
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Every line is a JSON object with a timestamp, a level and its fields. A
/// request writes an INFO line with the status and the latency, inside a span
/// that carries the method and the URI.
#[test]
fn request_is_logged_as_json_lines() {
    // The test logs one request into a buffer and checks the lines.
    //
    // 1. We install the subscriber as the global default, because the server
    //    runs on the runtime's threads, which a thread-local one misses.
    // 2. We serve the router and request `/healthz`, on SQLite alone, because
    //    the format does not depend on the backend.
    // 3. We check the shape of every line.
    // 4. We find the line of the request and check its fields and its span.

    // 1. install the subscriber
    let buffer = Buffer::default();
    // The filter that `logging::init` builds at `info`, the default of the
    // `log.level` setting.
    let filter = EnvFilter::default().add_directive(LevelFilter::INFO.into());
    let writer = buffer.clone();
    // Each event gets a clone of the buffer as its writer.
    tracing::subscriber::set_global_default(json_subscriber(filter, move || writer.clone()))
        .expect("no subscriber is installed yet");

    // 2. serve and request /healthz
    let server = serve_in_process(Backend::Sqlite);
    assert_eq!(get(server.addr, "/healthz").0, 200);

    // 3. check every line
    let lines = buffer.lines();
    for line in &lines {
        assert!(line["timestamp"].is_string(), "{line}");
        assert!(line["level"].is_string(), "{line}");
        assert!(line["fields"].is_object(), "{line}");
    }
    // 4. find the request line
    let request = lines
        .iter()
        // tower-http's `DefaultOnResponse` writes this message when the
        // router has made the response, before its body goes out.
        .find(|line| line["fields"]["message"] == "finished processing request")
        .unwrap_or_else(|| panic!("no request line in {lines:?}"));
    // The router raises tower-http's event from DEBUG to INFO.
    assert_eq!(request["level"], "INFO");
    assert_eq!(request["fields"]["status"], 200);
    // tower-http writes the latency with its unit, as `3 ms`.
    assert!(request["fields"]["latency"].is_string(), "{request}");
    // The trace layer puts the method and the URI on the request's span.
    assert_eq!(request["span"]["method"], "GET");
    assert_eq!(request["span"]["uri"], "/healthz");
}
