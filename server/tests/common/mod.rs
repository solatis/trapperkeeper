//! Helpers for the tests that run the binary as a process.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How long a start or a stop may take before a test gives up. Not measured;
/// generous for a loaded CI runner.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// A running `trapperkeeper serve`, killed when dropped so a failing assertion
/// leaves no process behind.
pub struct Server {
    pub child: Child,
    pub addr: SocketAddr,
}

impl Drop for Server {
    fn drop(&mut self) {
        // A child that already exited makes both calls fail, which is fine:
        // the shutdown test waits for the exit itself.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Starts `serve` on a free loopback port and returns once the port accepts a
/// connection. Panics when the server does not come up within `TIMEOUT` or
/// exits first. `stdout` is wired as given: a test that reads the log lines
/// pipes it and drains it, the others pass `Stdio::null()`, because an
/// undrained pipe blocks the child once it is full.
pub fn spawn_server(extra_args: &[&str], stdout: Stdio) -> Server {
    // A race and a timeout, both deliberate:
    //
    // 1. pick a free port;
    // 2. spawn the server on it;
    // 3. wait until the port accepts a connection.

    // 1. Pick a free port by binding port 0 and dropping the listener. The port
    //    can be taken between the drop and the child's bind; accepted as
    //    unlikely on loopback, and the binary has no output yet from which to
    //    read a port it chose itself.
    let addr = TcpListener::bind("127.0.0.1:0")
        .expect("a free port")
        .local_addr()
        .expect("the bound address");

    // 2. Spawn `serve --listen <addr>`. Passing `--listen` is also the
    //    end-to-end check of the flag layer that the command line tests cannot
    //    make, because a placeholder shows no resolved setting.
    let child = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .arg("serve")
        .arg("--listen")
        .arg(addr.to_string())
        .args(extra_args)
        .stdout(stdout)
        .spawn()
        .expect("the binary spawns");
    let mut server = Server { child, addr };

    // 3. Poll a TCP connect until it succeeds, so the test does not need the
    //    server's output to know it is up. A server that exits early fails
    //    the test now instead of at the timeout.
    let deadline = Instant::now() + TIMEOUT;
    while TcpStream::connect(addr).is_err() {
        if let Some(status) = server.child.try_wait().expect("the child can be polled") {
            panic!("the server exited before listening: {status}");
        }
        assert!(
            Instant::now() < deadline,
            "the server did not listen within {TIMEOUT:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    server
}

/// One `GET` over a fresh connection; returns the status code and the body.
pub fn get(addr: SocketAddr, path: &str) -> (u16, String) {
    // Hand-rolled HTTP/1.1, because two literal bodies and one small document
    // do not justify an HTTP client dependency. Two assumptions:
    //
    // 1. `Connection: close` makes the server close after the response, so
    //    reading to EOF ends it.
    let mut stream = TcpStream::connect(addr).expect("the server accepts");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .expect("the request is written");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("the response is read");

    // 2. The response is small and carries a content length, not chunks, so
    //    the body is everything after the blank line, and the status code is
    //    the second word of the first line.
    let (head, body) = response
        .split_once("\r\n\r\n")
        .expect("a header block ends with a blank line");
    let status = head
        .split_whitespace()
        .nth(1)
        .expect("a status code")
        .parse()
        .expect("a numeric status code");
    (status, body.to_owned())
}
