//! The shape of the log lines: the JSON subscriber on a buffer, the router
//! served in this process, one request.

mod common;

use std::io;
use std::sync::{Arc, Mutex};

use common::{Backend, get, serve_in_process};
use serde_json::Value;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use trapperkeeper_server::json_subscriber;

/// A writer onto a buffer the test reads back.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Buffer {
    /// Everything written so far, one parsed JSON object per line.
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

/// Every line is a JSON object with a timestamp, a level and its fields, and
/// a request is one INFO line with the status and the latency inside a span
/// that carries the method and the URI.
#[test]
fn request_is_logged_as_json_lines() {
    // 1. install the JSON subscriber on a buffer as the global default
    // 2. serve the router on SQLite and GET /healthz
    // 3. parse the buffer's lines and find the request line

    // 1. The global default, because the server runs on the runtime's
    //    threads, which a thread-local default does not reach.
    let buffer = Buffer::default();
    let filter = EnvFilter::default().add_directive(LevelFilter::INFO.into());
    let writer = buffer.clone();
    tracing::subscriber::set_global_default(json_subscriber(filter, move || writer.clone()))
        .expect("no subscriber is installed yet");

    // 2. SQLite alone, because the format does not depend on the backend.
    let server = serve_in_process(Backend::Sqlite);
    assert_eq!(get(server.addr, "/healthz").0, 200);

    // 3.
    let lines = buffer.lines();
    for line in &lines {
        assert!(line["timestamp"].is_string(), "{line}");
        assert!(line["level"].is_string(), "{line}");
        assert!(line["fields"].is_object(), "{line}");
    }
    let request = lines
        .iter()
        .find(|line| line["fields"]["message"] == "finished processing request")
        .unwrap_or_else(|| panic!("no request line in {lines:?}"));
    assert_eq!(request["level"], "INFO");
    assert_eq!(request["fields"]["status"], 200);
    assert!(request["fields"]["latency"].is_string(), "{request}");
    assert_eq!(request["span"]["method"], "GET");
    assert_eq!(request["span"]["uri"], "/healthz");
}
