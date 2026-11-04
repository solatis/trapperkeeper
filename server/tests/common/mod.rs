//! Helpers for the server tests: test databases on each backend, the router
//! served in this process, the binary started as a process, and a GET.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code)]

use std::env;
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use sqlx::migrate::MigrateDatabase;
use sqlx::mysql::MySql;
use sqlx::postgres::Postgres;
use sqlx::sqlite::Sqlite;
use trapperkeeper_server::{apply_migrations, create_store, router};
use url::Url;

/// How long a start or a stop may take before a test gives up. Not measured;
/// generous for a loaded CI runner.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// Numbers the scratch directories of this process.
static SCRATCH: AtomicU32 = AtomicU32::new(0);

/// A directory of its own for one test, removed when dropped.
pub struct Scratch {
    pub path: PathBuf,
}

impl Scratch {
    /// Creates an empty directory under the system temporary directory.
    /// Panics when it cannot.
    pub fn create() -> Self {
        // The process id keeps test binaries apart, and the counter keeps the
        // parallel tests of one binary apart.
        let path = std::env::temp_dir().join(format!(
            "trapperkeeper-test-{}-{}",
            std::process::id(),
            SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("the scratch directory is created");
        Self { path }
    }

    /// The URL of a SQLite database file named `name` in this directory.
    pub fn sqlite_url(&self, name: &str) -> String {
        format!("sqlite://{}", self.path.join(name).display())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // A directory left behind in the temporary directory fails no test.
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// A database backend a test runs against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Sqlite,
    Postgres,
    MariaDb,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
            Self::MariaDb => "mariadb",
        })
    }
}

/// The variable that holds the admin URL of a server backend. The names do
/// not start with `TRAPPERKEEPER_`, which the binary rejects as unknown
/// settings.
fn admin_url_var(backend: Backend) -> Option<&'static str> {
    match backend {
        Backend::Sqlite => None,
        Backend::Postgres => Some("TK_TEST_POSTGRES_URL"),
        Backend::MariaDb => Some("TK_TEST_MARIADB_URL"),
    }
}

/// The backends this run tests: SQLite always, Postgres and MariaDB when
/// their admin URL is set. Panics when `TK_TEST_EXPECT` names a backend whose
/// URL is absent.
pub fn backends() -> Vec<Backend> {
    let all = [Backend::Sqlite, Backend::Postgres, Backend::MariaDb];
    let reachable: Vec<Backend> = all
        .into_iter()
        .filter(|b| admin_url_var(*b).is_none_or(|var| env::var_os(var).is_some()))
        .collect();
    // A missing backend fails the run instead of being skipped, because a
    // skipped test passes (doc/adr/0021).
    let expected = env::var("TK_TEST_EXPECT").unwrap_or_default();
    for name in expected.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        assert!(
            reachable.iter().any(|b| b.to_string() == name),
            "TK_TEST_EXPECT names {name}, and its admin URL is not set"
        );
    }
    reachable
}

/// Runs `body` once per backend from `backends`. Each run first prints the
/// backend, so the captured output of a failing test names it.
pub fn on_each_backend(body: impl Fn(Backend)) {
    for backend in backends() {
        eprintln!("backend: {backend}");
        body(backend);
    }
}

/// Numbers the server databases of this process.
static DATABASE: AtomicU32 = AtomicU32::new(0);

/// A database URL of its own for one test, on one backend. Nothing exists at
/// the URL until migrate creates it, and whatever exists is dropped with the
/// value.
pub struct TestDatabase {
    pub backend: Backend,
    pub url: String,
    // Holds the SQLite file, and is `None` on a server backend.
    scratch: Option<Scratch>,
}

impl TestDatabase {
    /// Reserves a database on `backend`: a file in a scratch directory for
    /// SQLite, a database named after the process and a counter on a server.
    pub fn fresh(backend: Backend) -> Self {
        let Some(var) = admin_url_var(backend) else {
            let scratch = Scratch::create();
            let url = scratch.sqlite_url("store.db");
            return Self {
                backend,
                url,
                scratch: Some(scratch),
            };
        };
        let admin = env::var(var).expect("the admin URL is set");
        let mut url = Url::parse(&admin).expect("the admin URL parses");
        url.set_path(&format!(
            "tk_test_{}_{}",
            std::process::id(),
            DATABASE.fetch_add(1, Ordering::Relaxed)
        ));
        Self {
            backend,
            url: url.to_string(),
            scratch: None,
        }
    }

    /// Whether a database exists at the URL.
    pub fn exists(&self) -> bool {
        let url = self.url.as_str();
        block_on(async {
            match self.backend {
                Backend::Sqlite => Sqlite::database_exists(url).await,
                Backend::Postgres => Postgres::database_exists(url).await,
                Backend::MariaDb => MySql::database_exists(url).await,
            }
        })
        .expect("the database server answers")
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // The scratch directory takes a SQLite file with it.
        if self.scratch.is_some() {
            return;
        }
        // A server database left behind, or one that was never created, fails
        // no test, so the result is ignored. Postgres drops by force, because
        // a killed server's connections may not have closed yet.
        let url = self.url.as_str();
        let _ = block_on(async {
            match self.backend {
                Backend::Sqlite => Ok(()),
                Backend::Postgres => Postgres::force_drop_database(url).await,
                Backend::MariaDb => MySql::drop_database(url).await,
            }
        });
    }
}

/// Runs `future` to completion on a runtime of its own, because the tests are
/// synchronous and sqlx is not.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime")
        .block_on(future)
}

/// The production router on a freshly migrated database of its own on one
/// backend, served in this process on `addr`. Dropping it stops the server
/// and then drops the database.
pub struct InProcess {
    pub addr: SocketAddr,
    // Declared before `database`, so the server and its pool stop before the
    // database is dropped.
    runtime: tokio::runtime::Runtime,
    database: TestDatabase,
}

/// Serves the router on `backend` in this process and returns once the
/// address accepts connections. Starts no process. Panics when the database
/// cannot be created or migrated.
pub fn serve_in_process(backend: Backend) -> InProcess {
    // 1. create and migrate a database of its own through the library
    // 2. bind a listener on port 0 and keep it
    // 3. serve the router on that listener, on a runtime the value owns

    // 1. A multi-thread runtime, so the server runs on its own threads while
    //    the test blocks in `get`.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let database = TestDatabase::fresh(backend);
    let store = runtime.block_on(async {
        let store = create_store(&database.url)
            .await
            .expect("the database is created");
        apply_migrations(&store)
            .await
            .expect("the migrations apply");
        store
    });

    // 2. The listener stays open from the bind to the drop, so a fork on
    //    another test thread shares it instead of holding the last copy.
    //    Connections queue in its backlog from the bind on, so no wait is needed.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback listener");
    let addr = listener.local_addr().expect("the bound address");

    // 3. tokio adopts the listener only in non-blocking mode and inside a
    //    runtime.
    listener
        .set_nonblocking(true)
        .expect("the listener turns non-blocking");
    let listener = {
        let _context = runtime.enter();
        tokio::net::TcpListener::from_std(listener).expect("tokio adopts the listener")
    };
    runtime.spawn(axum::serve(listener, router(store)).into_future());

    InProcess {
        addr,
        runtime,
        database,
    }
}

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
    // Fields drop after `Drop::drop` has stopped the process, so the server
    // no longer holds the database when it is dropped.
    pub database: Option<TestDatabase>,
}

impl Drop for Server {
    fn drop(&mut self) {
        // A child that already exited makes both calls fail, which is fine:
        // the shutdown test waits for the exit itself.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// How a started `serve` came out.
pub enum Launch {
    /// It listens on its port.
    Listening(Server),
    /// It exited before listening, with this status and stderr.
    Exited(ExitStatus, String),
}

/// Starts `serve` with `args` on a free loopback port and returns once the
/// port accepts a connection or the process exits. Panics when neither
/// happens within `TIMEOUT`. `stdout` is wired as given: a test that reads
/// the log lines pipes it and drains it, the others pass `Stdio::null()`,
/// because an undrained pipe blocks the child once it is full.
pub fn launch(args: &[&str], stdout: Stdio) -> Launch {
    // Three steps under one lock, with a timeout on the last:
    //
    // 1. pick a free port;
    // 2. spawn the server on it;
    // 3. wait until the port accepts a connection or the server exits.
    //
    // The lock serializes the tests of this process from the pick to the
    // listening server, because the pick leaves a window in which the port is
    // free and another test would be handed the same one. A poisoned lock is
    // taken anyway: the test that panicked holding it left nothing to protect.
    let _spawning = SPAWN.lock().unwrap_or_else(PoisonError::into_inner);

    // 1. Pick a free port by binding port 0 and dropping the listener. Between
    //    the drop and the child's bind an unrelated process could take the
    //    port; accepted as unlikely on loopback.
    let addr = TcpListener::bind("127.0.0.1:0")
        .expect("a free port")
        .local_addr()
        .expect("the bound address");

    // 2. Spawn `serve --listen <addr>`. stderr is piped and read only after
    //    an exit, because the server writes to it only when it fails.
    let child = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .arg("serve")
        .arg("--listen")
        .arg(addr.to_string())
        .args(args)
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary spawns");
    let mut server = Server {
        child,
        addr,
        database: None,
    };

    // 3. Poll a TCP connect until it succeeds, so the test does not need the
    //    server's output to know it is up. The exit check runs once more after
    //    the connect, so a connection to someone else's server on the port is
    //    not mistaken for ours.
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(status) = server.child.try_wait().expect("the child can be polled") {
            return Launch::Exited(status, stderr_of(&mut server.child));
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
    Launch::Listening(server)
}

/// Everything an exited child wrote to its piped stderr.
fn stderr_of(child: &mut Child) -> String {
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr is piped")
        .read_to_string(&mut stderr)
        .expect("stderr is read");
    stderr
}

/// Starts `serve` like `launch` on a freshly migrated database of its own on
/// `backend`, and returns the listening server. Panics when the server exits
/// before listening.
pub fn spawn_server(backend: Backend, extra_args: &[&str], stdout: Stdio) -> Server {
    // serve refuses a database that migrate has not built.
    let database = TestDatabase::fresh(backend);
    migrate_database(&database);
    let mut args = vec!["--database-url", database.url.as_str()];
    args.extend_from_slice(extra_args);
    match launch(&args, stdout) {
        Launch::Listening(mut server) => {
            server.database = Some(database);
            server
        }
        Launch::Exited(status, stderr) => {
            panic!("the server exited before listening: {status}\n{stderr}")
        }
    }
}

/// Runs migrate on `database`. Panics when migrate fails.
pub fn migrate_database(database: &TestDatabase) {
    let out = run_migrate(&database.url);
    assert!(
        out.status.success(),
        "migrate failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Runs `migrate` on the database at `url` and returns its output.
pub fn run_migrate(url: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .arg("migrate")
        .arg("--database-url")
        .arg(url)
        .output()
        .expect("the binary runs")
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
