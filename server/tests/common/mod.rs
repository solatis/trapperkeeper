//! Helpers for the server tests: test databases on each backend, a pool on
//! one of them, the router served in this process with its superadmin and a
//! keyring, the binary started as a process, HTTP requests, and calls on the
//! management routes as a logged-in user.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code, unused_imports, unused_macros)]

use std::env;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sqlx::migrate::MigrateDatabase;
use sqlx::mysql::MySql;
use sqlx::postgres::Postgres;
use sqlx::sqlite::Sqlite;
use trapperkeeper_server::{AppState, Keyring, Store, router, setup};
use url::Url;

/// How long a start or a stop may take before a test gives up. Not measured;
/// generous for a loaded CI runner.
pub const TIMEOUT: Duration = Duration::from_secs(5);

/// The signing passphrase of every test server, fixed so a failing run
/// reproduces. 32 characters or more, as the setting requires.
pub const TEST_KEY: &str = "the test signing passphrase of trapperkeeper";

/// A second passphrase, for the rotation cases.
pub const TEST_PREVIOUS_KEY: &str = "the previous test signing passphrase of trapperkeeper";

/// The system tenant's name, which every in-process login names.
pub const SYSTEM_TENANT: &str = "$tk";

/// The superadmin every in-process server seeds.
pub const ADMIN: &str = "admin";

/// The id of the system tenant, as the first migration seeds it.
pub const SYSTEM_TENANT_ID: &str = "00000000-0000-7000-8000-000000000000";

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
/// the URL until migrate or serve creates it, and whatever exists is dropped
/// with the value.
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

/// Evaluates `$body` with `$pool` bound to a pool on `$database`, whichever
/// backend it is on. A macro, because each backend's pool is its own type and
/// `$body` is checked once per type.
macro_rules! on_pool {
    ($database:expr, |$pool:ident| $body:expr) => {{
        let url = $database.url.as_str();
        $crate::common::block_on(async {
            match $database.backend {
                $crate::common::Backend::Sqlite => {
                    let $pool = ::sqlx::sqlite::SqlitePool::connect(url)
                        .await
                        .expect("the database opens");
                    let result = $body;
                    $pool.close().await;
                    result
                }
                $crate::common::Backend::Postgres => {
                    let $pool = ::sqlx::postgres::PgPool::connect(url)
                        .await
                        .expect("the database opens");
                    let result = $body;
                    $pool.close().await;
                    result
                }
                $crate::common::Backend::MariaDb => {
                    let $pool = ::sqlx::mysql::MySqlPool::connect(url)
                        .await
                        .expect("the database opens");
                    let result = $body;
                    $pool.close().await;
                    result
                }
            }
        })
    }};
}

pub(crate) use on_pool;

/// `statement` with its numbered placeholders as `backend` spells them:
/// MariaDB takes `?`, in the same order.
pub fn placeholders(backend: Backend, statement: &'static str) -> String {
    match backend {
        Backend::MariaDb => (1..=9).fold(statement.to_owned(), |s, n| {
            s.replace(&format!("${n}"), "?")
        }),
        Backend::Sqlite | Backend::Postgres => statement.to_owned(),
    }
}

/// The production router on a database of its own on one backend, set up
/// with the superadmin `ADMIN` and the keys derived from `TEST_KEY`, served
/// in this process on `addr`. Dropping it stops the server and then drops the
/// database.
pub struct InProcess {
    pub addr: SocketAddr,
    /// The generated password of `ADMIN`.
    pub admin_password: String,
    /// The store the server runs on, for a test that derives keys of its own.
    pub store: Store,
    // Declared before `database`, so the server and its pool stop before the
    // database is dropped.
    runtime: tokio::runtime::Runtime,
    database: TestDatabase,
}

impl InProcess {
    /// The database the server runs on, for reads and writes beside it.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }
}

/// Sets up `database` the way migrate does and returns its store and the
/// superadmin's one-time password. Panics when the set-up fails or created no
/// superadmin.
pub fn set_up_database(database: &TestDatabase) -> (Store, String) {
    // The store outlives the runtime `block_on` builds: the pool tests each
    // connection before handing it out and reopens it on the caller's runtime.
    let (store, set_up) = block_on(setup::create(&database.url)).expect("the database is set up");
    let password = set_up
        .password
        .expect("the set-up of a fresh database creates the superadmin");
    (store, password)
}

/// Serves the router on `store`, whose database is `database` and whose
/// superadmin's password is `admin_password`, with the keys derived from
/// `TEST_KEY`, and returns once the address accepts connections. Panics when
/// the keys do not derive.
pub fn serve_store(database: TestDatabase, store: Store, admin_password: String) -> InProcess {
    // 1. derive the keys on a runtime the value owns
    // 2. bind a listener on port 0 and keep it
    // 3. serve the router on that listener

    // 1. A multi-thread runtime, so the server runs on its own threads while
    //    the test blocks in a request.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let keyring = runtime
        .block_on(Keyring::from_store(&store, TEST_KEY, None))
        .expect("the keys derive");
    let state = AppState {
        store: store.clone(),
        keyring: std::sync::Arc::new(keyring),
    };

    // 2. The listener stays open from the bind to the drop, so a fork on
    //    another test thread shares it instead of holding the last copy.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback listener");
    // Connections queue in the backlog from the bind on, so no wait is needed.
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
    // With connect info, as serve does, because the audit trail records the
    // peer's address.
    runtime.spawn(
        axum::serve(
            listener,
            router(state).into_make_service_with_connect_info::<SocketAddr>(),
        )
        .into_future(),
    );

    InProcess {
        addr,
        admin_password,
        store,
        runtime,
        database,
    }
}

/// The production router on a freshly set-up database of its own on
/// `backend`, served in this process. Panics when the set-up fails.
pub fn serve_in_process(backend: Backend) -> InProcess {
    let database = TestDatabase::fresh(backend);
    let (store, admin_password) = set_up_database(&database);
    serve_store(database, store, admin_password)
}

/// A running `trapperkeeper serve`, killed when dropped so a failing assertion
/// leaves no process behind.
pub struct Server {
    pub child: Child,
    pub addr: SocketAddr,
    /// The log lines up to and including "listening", parsed.
    pub startup: Vec<Value>,
    /// The log lines after "listening", forwarded by a drain thread.
    pub lines: Receiver<String>,
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

/// Starts `serve` with `args` on port 0, with stdout piped and drained, and
/// returns once the "listening" line names the address or the process exits.
/// Panics when neither happens within `TIMEOUT`.
pub fn launch(args: &[&str]) -> Launch {
    // 1. spawn `serve --listen 127.0.0.1:0` with stdout and stderr piped
    // 2. forward stdout line by line on a thread
    // 3. read lines until "listening" or the end of stdout

    // 1. The server binds port 0 itself and keeps the socket, so no other
    //    process ever holds the port first. stderr is read only after an
    //    exit, because the server writes to it only when it fails. Every
    //    serve needs a signing key, given through the environment.
    let mut child = Command::new(env!("CARGO_BIN_EXE_trapperkeeper"))
        .env("TRAPPERKEEPER_AUTH__SIGNING_KEY", TEST_KEY)
        .arg("serve")
        .arg("--listen")
        .arg("127.0.0.1:0")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary spawns");

    // 2. An undrained pipe blocks the server once it is full.
    let lines = drain(child.stdout.take().expect("stdout is piped"));

    // 3. The end of stdout means the process exited, and its status and
    //    stderr are returned.
    let Some(startup) = read_until(&lines, "listening") else {
        let status = child.wait().expect("the child is waited for");
        return Launch::Exited(status, stderr_of(&mut child));
    };
    // The line's `address` holds the port the server bound, not 0.
    let addr = startup.last().expect("the listening line")["fields"]["address"]
        .as_str()
        .expect("an address field")
        .parse()
        .expect("a socket address");
    Launch::Listening(Server {
        child,
        addr,
        startup,
        lines,
        database: None,
    })
}

/// Forwards each line of `stdout` on a thread of its own, so a test can wait
/// for a line with a deadline instead of blocking on a read that may never
/// return.
fn drain(stdout: ChildStdout) -> Receiver<String> {
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
    receiver
}

/// Reads lines until one whose message is `message`, each parsed as JSON, and
/// returns all of them, or `None` when stdout ends first. Panics when
/// `TIMEOUT` passes first.
fn read_until(lines: &Receiver<String>, message: &str) -> Option<Vec<Value>> {
    let deadline = Instant::now() + TIMEOUT;
    let mut read = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let line = match lines.recv_timeout(remaining) {
            Ok(line) => line,
            Err(RecvTimeoutError::Disconnected) => return None,
            Err(RecvTimeoutError::Timeout) => {
                panic!("no line with message {message:?} within {TIMEOUT:?}")
            }
        };
        let value: Value = serde_json::from_str(&line).expect("one JSON object per line");
        let found = value["fields"]["message"] == message;
        read.push(value);
        if found {
            return Some(read);
        }
    }
}

/// Reads lines until one whose message is `message`, each parsed as JSON, and
/// returns all of them. Panics when `TIMEOUT` or the end of stdout comes
/// first.
pub fn lines_until(lines: &Receiver<String>, message: &str) -> Vec<Value> {
    read_until(lines, message)
        .unwrap_or_else(|| panic!("stdout ended before a line with message {message:?}"))
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
pub fn spawn_server(backend: Backend, extra_args: &[&str]) -> Server {
    let database = TestDatabase::fresh(backend);
    set_up_database(&database);
    let mut args = vec!["--database-url", database.url.as_str()];
    args.extend_from_slice(extra_args);
    match launch(&args) {
        Launch::Listening(mut server) => {
            server.database = Some(database);
            server
        }
        Launch::Exited(status, stderr) => {
            panic!("the server exited before listening: {status}\n{stderr}")
        }
    }
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

/// One response: the status code, the headers as sent, and the body.
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Response {
    /// The values of every header named `name`, case-insensitively.
    pub fn header_values(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }
}

/// One `GET` over a fresh connection; returns the status code and the body.
pub fn get(addr: SocketAddr, path: &str) -> (u16, String) {
    let response = request(addr, "GET", path, &[], None);
    (response.status, response.body)
}

/// One request over a fresh connection, with `headers` as given and `body`
/// sent as JSON when there is one. Returns the response.
pub fn request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Response {
    // Hand-rolled HTTP/1.1, because a few small requests do not justify an
    // HTTP client dependency.
    //
    // 1. One request with `Connection: close`, so the server closes once it
    //    has answered and nothing else is ever sent on the connection.
    let mut stream = TcpStream::connect(addr).expect("the server accepts");
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (name, value) in headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    let body = body.unwrap_or_default();
    if !body.is_empty() {
        let _ = write!(
            head,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    write!(stream, "{head}\r\n{body}").expect("the request is written");

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
    let headers: Vec<(String, String)> = head
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_owned(), value.trim().to_owned()))
        .collect();
    let length = header(&head, "content-length")
        .expect("an announced content length")
        .parse()
        .expect("a numeric content length");
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("the body is read");
    Response {
        status,
        headers,
        body: String::from_utf8(body).expect("a UTF-8 body"),
    }
}

/// The value of the first header named `name`, case-insensitively, trimmed.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then_some(value.trim())
    })
}

/// Logs in to `tenant` as `name` with `password`, and returns the access
/// token and whether the password must change. Panics when the login fails.
pub fn log_in(server: &InProcess, tenant: &str, name: &str, password: &str) -> (String, bool) {
    let credentials = json!({ "tenant": tenant, "name": name, "password": password });
    let response = request(
        server.addr,
        "POST",
        "/api/v1/login",
        &[],
        Some(&credentials.to_string()),
    );
    assert_eq!(response.status, 200, "{}", response.body);
    let login: Value = serde_json::from_str(&response.body).expect("a JSON body");
    (
        login["access_token"]
            .as_str()
            .expect("an access token")
            .to_owned(),
        login["password_must_change"].as_bool().expect("a flag"),
    )
}

/// The superadmin's access token.
pub fn superadmin_token(server: &InProcess) -> String {
    log_in(server, SYSTEM_TENANT, ADMIN, &server.admin_password).0
}

/// Sends `method` to `path` with `token` as the bearer, and `body` as JSON
/// when there is one.
pub fn call(
    server: &InProcess,
    token: &str,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> Response {
    let bearer = format!("Bearer {token}");
    let body = body.map(Value::to_string);
    request(
        server.addr,
        method,
        path,
        &[("Authorization", &bearer)],
        body.as_deref(),
    )
}
