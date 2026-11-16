//! Helpers for the server tests: the backends a run tests, which
//! `TK_TEST_EXPECT` can require, test databases on each backend, a pool on
//! one of them, the rewrite of numbered placeholders for MariaDB, a database
//! set up the way migrate does, the router served in this process with its
//! superadmin and a keyring, HTTP requests, and calls on the management routes
//! as a logged-in user.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code, unused_imports, unused_macros)]

use std::env;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{Value, json};
use sqlx::migrate::MigrateDatabase;
use sqlx::mysql::MySql;
use sqlx::postgres::Postgres;
use sqlx::sqlite::Sqlite;
use trapperkeeper_server::{AppState, Keyring, Store, router, setup};
use url::Url;

/// The passphrase that every in-process server derives its keys from. A test
/// derives the same keys from the server's store with it. It has 32
/// characters or more, as a signing key in the settings must.
pub const TEST_KEY: &str = "the test signing passphrase of trapperkeeper";

/// A second passphrase, for the rotation cases.
pub const TEST_PREVIOUS_KEY: &str = "the previous test signing passphrase of trapperkeeper";

/// The name of the system tenant, which the first migration seeds and which
/// the superadmin logs in to. The library exports no constant for it.
pub const SYSTEM_TENANT: &str = "$tk";

/// The user name of the superadmin, as `superadmin::NAME` has it. The set-up
/// of a database creates this user in the system tenant before any server
/// runs. The library does not export the constant.
pub const ADMIN: &str = "admin";

/// The id of the system tenant, as the first migration seeds it. It copies
/// `store::SYSTEM_TENANT_ID`, which the library does not export.
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
        // The spellings are the backend names that `TK_TEST_EXPECT` lists, and
        // `backends` compares each name in it with them.
        f.write_str(match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
            Self::MariaDb => "mariadb",
        })
    }
}

/// The name of the environment variable that holds the admin URL of a server
/// backend, and `None` for SQLite. An admin URL names a user who may create
/// and drop databases, and each test derives from it the URL of a database of
/// its own.
fn admin_url_var(backend: Backend) -> Option<&'static str> {
    match backend {
        Backend::Sqlite => None,
        // The names avoid the prefix `TRAPPERKEEPER_`, because the settings
        // loader reads every variable under that prefix as a setting and
        // refuses a key it does not know.
        Backend::Postgres => Some("TK_TEST_POSTGRES_URL"),
        Backend::MariaDb => Some("TK_TEST_MARIADB_URL"),
    }
}

/// The backends this run tests: SQLite always, Postgres and MariaDB when
/// their admin URL is set. Panics when `TK_TEST_EXPECT` names an unknown
/// backend or one whose URL is absent.
pub fn backends() -> Vec<Backend> {
    let all = [Backend::Sqlite, Backend::Postgres, Backend::MariaDb];
    let configured: Vec<Backend> = all
        .into_iter()
        .filter(|b| admin_url_var(*b).is_none_or(|var| env::var_os(var).is_some()))
        .collect();
    // A backend that `TK_TEST_EXPECT` names fails the run when its admin URL
    // is not set, because a skipped test passes (ADR-0021).
    let expected = env::var("TK_TEST_EXPECT").unwrap_or_default();
    for name in expected.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        assert!(
            all.iter().any(|b| b.to_string() == name),
            "TK_TEST_EXPECT names {name}, which is none of sqlite, postgres and mariadb"
        );
        assert!(
            configured.iter().any(|b| b.to_string() == name),
            "TK_TEST_EXPECT names {name}, and its admin URL is not set"
        );
    }
    configured
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
/// the URL until `setup::create` or `setup::open_or_set_up` creates it, and
/// whatever exists is dropped with the value.
pub struct TestDatabase {
    pub backend: Backend,
    pub url: String,
    // Holds the SQLite file, and is `None` on a server backend.
    scratch: Option<Scratch>,
}

impl TestDatabase {
    /// Reserves a database URL of its own on `backend` and creates nothing
    /// there. On SQLite the URL names a file in a scratch directory. Panics
    /// when the admin URL variable of a server backend is not set or does not
    /// hold a URL.
    pub fn fresh(backend: Backend) -> Self {
        // Only SQLite has no admin URL variable.
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
        // The path names the database, so the URL keeps the admin's user,
        // host and port. The process id and the counter keep parallel tests
        // apart, and `tk_test_` marks a database left behind.
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

    /// Whether a database exists at the URL. Panics when the database server
    /// does not answer.
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
        let url = self.url.as_str();
        // A server database left behind, or one that was never created, fails
        // no test, so the result is ignored.
        let _ = block_on(async {
            match self.backend {
                // Never reached: SQLite returned above with its scratch
                // directory.
                Backend::Sqlite => Ok(()),
                // Postgres drops by force, because the connections of a stopped
                // server may not have closed yet. sqlx offers no forced drop
                // for MariaDB.
                Backend::Postgres => Postgres::force_drop_database(url).await,
                Backend::MariaDb => MySql::drop_database(url).await,
            }
        });
    }
}

/// The runtime that the whole test process shares, built on the first call.
/// Panics when it cannot be built.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    // A sqlx connection fails once the runtime that opened it shuts down, and
    // a test opens a store in one `block_on` and uses it in the next.
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        // The runtime has several threads, so an in-process server keeps
        // running while its test blocks in a request.
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("a runtime")
    })
}

/// Runs `future` to completion on the process's runtime, because the tests
/// are synchronous and sqlx is not.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    runtime().block_on(future)
}

/// Evaluates `$body` with `$pool` bound to a pool on `$database`, whichever
/// backend it is on, and returns its value. `$body` may await. The macro
/// closes the pool afterwards, so no test leaves connections behind for
/// another. Panics when the database does not open.
macro_rules! on_pool {
    ($database:expr, |$pool:ident| $body:expr) => {{
        let url = $database.url.as_str();
        $crate::common::block_on(async {
            // Each arm repeats `$body`, because each pool is a type of its own
            // and the compiler checks `$body` once per pool type. A generic
            // function over the pool could not take such a body.
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

/// `statement` unchanged on SQLite and Postgres, and with each of `$1` to
/// `$9` replaced by `?` on MariaDB. MariaDB binds `?` by position, so the
/// caller writes each placeholder once, numbers them in ascending order of
/// appearance, and stops at `$9`. The callers pass literals, so they may pass
/// the result to `AssertSqlSafe`.
pub fn placeholders(backend: Backend, statement: &'static str) -> String {
    match backend {
        // The fold replaces in ascending order, so it stops at `$9`, or it
        // would turn a `$10` into `?0`.
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
    /// The store the server runs on, for a test that calls the library on the
    /// server's database, for example to derive key rings of its own.
    pub store: Store,
    server: tokio::task::JoinHandle<io::Result<()>>,
    database: TestDatabase,
}

impl InProcess {
    /// The database the server runs on, for reads and writes beside it.
    pub fn database(&self) -> &TestDatabase {
        &self.database
    }
}

impl Drop for InProcess {
    fn drop(&mut self) {
        // `abort` does not wait for the server's request tasks, which may
        // still hold connections of the pool that they share with this store.
        // Closing the pool waits for each, so none is open when the
        // `database` field drops.
        self.server.abort();
        block_on(async {
            match &self.store {
                Store::Sqlite(pool) => pool.close().await,
                Store::Postgres(pool) => pool.close().await,
                Store::MariaDb(pool) => pool.close().await,
            }
        });
    }
}

/// Sets up `database` the way migrate does and returns its store and the
/// superadmin's one-time password. Panics when the set-up fails or created no
/// superadmin.
pub fn set_up_database(database: &TestDatabase) -> (Store, String) {
    let (store, set_up) = block_on(setup::create(&database.url)).expect("the database is set up");
    let password = set_up
        .password
        .expect("the set-up of a fresh database creates the superadmin");
    (store, password)
}

/// Serves the router on `store`, whose database is `database` and whose
/// superadmin's password is `admin_password`, with the keys derived from
/// `TEST_KEY`, and returns once the address accepts connections. Panics when
/// the keys do not derive, when no loopback listener binds, or when tokio
/// cannot adopt the listener.
pub fn serve_store(database: TestDatabase, store: Store, admin_password: String) -> InProcess {
    // No previous passphrase, so only `TEST_KEY` signs and checks.
    let keyring = block_on(Keyring::from_store(&store, TEST_KEY, None)).expect("the keys derive");
    let state = AppState {
        store: store.clone(),
        keyring: std::sync::Arc::new(keyring),
    };

    // The test binds port 0 itself and hands the listener to the server, so
    // it knows the address (ADR-0032). Connections queue from the bind on.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback listener");
    let addr = listener.local_addr().expect("the bound address");

    // tokio adopts only a non-blocking listener, inside a runtime context.
    listener
        .set_nonblocking(true)
        .expect("the listener turns non-blocking");
    let listener = {
        let _context = runtime().enter();
        tokio::net::TcpListener::from_std(listener).expect("tokio adopts the listener")
    };
    // The router gets connect info, as serve gives it, because the audit
    // trail records the peer's address.
    let server = runtime().spawn(
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
        server,
        database,
    }
}

/// The production router on a freshly set-up database of its own on
/// `backend`, served in this process. Panics when the admin URL of a server
/// backend is missing or does not parse, when the set-up fails, when the keys
/// do not derive or when no listener binds.
pub fn serve_in_process(backend: Backend) -> InProcess {
    let database = TestDatabase::fresh(backend);
    let (store, admin_password) = set_up_database(&database);
    serve_store(database, store, admin_password)
}

/// One response: the status code, the headers in the order the server sent
/// them, with their names as sent and their values trimmed, and the body.
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

/// Sends one `GET` over a fresh connection and returns the status code and
/// the body.
pub fn get(addr: SocketAddr, path: &str) -> (u16, String) {
    let response = request(addr, "GET", path, &[], None);
    (response.status, response.body)
}

/// Sends one request with `headers` over a fresh connection, with `body` as
/// JSON when it is given and not empty, and returns the response. Panics when
/// the server does not accept the connection, or when the response has no
/// numeric status code, no numeric `Content-Length` or a body that is not
/// UTF-8.
pub fn request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Response {
    // The function speaks HTTP/1.1 by hand, because a few small requests do
    // not justify an HTTP client crate.
    //
    // 1. We write one request with `Connection: close`, so the server closes
    //    the connection after its answer and no idle connection outlives the
    //    call or serves another request.
    // 2. We read the head up to its blank line.
    // 3. We take the status code and the headers from the head.
    // 4. We read as many body bytes as `Content-Length` announces. Reading to
    //    the end of the stream can fail with "connection reset by peer" after
    //    the whole response has arrived, which says nothing about the server.

    // 1. write the request
    let mut stream = TcpStream::connect(addr).expect("the server accepts");
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (name, value) in headers {
        // Writing into a `String` cannot fail.
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

    // 2. read the head
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("the head is read");
        // `read_line` leaves `line` empty only at the end of the stream, so
        // the loop also ends when the server closes before the blank line.
        if line == "\r\n" || line.is_empty() {
            break;
        }
        head.push_str(&line);
    }
    // 3. take the status and the headers
    // The second word of the status line, as `200` of `HTTP/1.1 200 OK`.
    let status = head
        .split_whitespace()
        .nth(1)
        .expect("a status code")
        .parse()
        .expect("a numeric status code");
    // Every line after the status line that holds a colon.
    let headers: Vec<(String, String)> = head
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_owned(), value.trim().to_owned()))
        .collect();
    // 4. read the body by its length
    // Every route answers with a whole body, a literal or one JSON document,
    // which axum sends with its length.
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
/// token and whether the password must change. The flag restricts no route.
/// Panics when the login fails.
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
