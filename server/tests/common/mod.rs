//! Helpers for the tests that run the binary as a process.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// How long a start or a stop may take before a test gives up. Not measured;
/// generous for a loaded CI runner.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// Held from the port pick until the server listens, so two tests of this
/// process cannot be handed the same port. Without it two were, on the Linux
/// CI runner: the loser's server failed to bind, and its test talked to the
/// winner's server until that one was killed (run 41285437744).
static SPAWN: Mutex<()> = Mutex::new(());

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
    // Three steps under one lock, with a timeout on the last:
    //
    // 1. pick a free port;
    // 2. spawn the server on it;
    // 3. wait until the port accepts a connection.
    //
    // The lock serializes the tests of this process from the pick to the
    // listening server, because the pick leaves a window in which the port is
    // free and another test would be handed the same one. A poisoned lock is
    // taken anyway: the test that panicked holding it left nothing to protect.
    let _spawning = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);

    // 1. Pick a free port by binding port 0 and dropping the listener. Between
    //    the drop and the child's bind an unrelated process could take the
    //    port; accepted as unlikely on loopback, and the binary has no output
    //    yet from which to read a port it chose itself.
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
    //    the test now instead of at the timeout, and the check runs once more
    //    after the connect, so a connection to someone else's server on the
    //    port is not mistaken for ours.
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = server.child.try_wait().expect("the child can be polled") {
            panic!("the server exited before listening: {status}");
        }
        if TcpStream::connect(addr).is_ok() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the server did not listen within {TIMEOUT:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        server
            .child
            .try_wait()
            .expect("the child can be polled")
            .is_none(),
        "the server exited after something else accepted on its port"
    );
    server
}

/// One `GET` over a fresh connection; returns the status code and the body.
pub fn get(addr: SocketAddr, path: &str) -> (u16, String) {
    // Hand-rolled HTTP/1.1, because two literal bodies and one small document
    // do not justify an HTTP client dependency.
    //
    // 1. One request with `Connection: close`, so the server closes once it
    //    has answered and nothing else is ever sent on the connection.
    let mut stream = TcpStream::connect(addr).expect("the server accepts");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .expect("the request is written");

    // 2. Read the head up to the blank line, then exactly the announced body
    //    length, and stop. Reading to the end of the stream instead failed on
    //    the Linux CI runner with "connection reset by peer" after the whole
    //    response had arrived (run 41285799929); a reset after the last
    //    announced byte says nothing about the server. The bodies are literals
    //    or one document, so a content length is always announced.
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("the head is read");
        if line == "\r\n" || line.is_empty() {
            break;
        }
        head.push_str(&line);
    }
    let status = head
        .split_whitespace()
        .nth(1)
        .expect("a status code")
        .parse()
        .expect("a numeric status code");
    let length = header(&head, "content-length")
        .expect("an announced content length")
        .parse()
        .expect("a numeric content length");
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("the body is read");
    (status, String::from_utf8(body).expect("a UTF-8 body"))
}

/// The value of the first header named `name`, case-insensitively, trimmed.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then_some(value.trim())
    })
}
