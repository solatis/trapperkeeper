# server/

**What**: the single server binary, `trapperkeeper`, built from the package
`trapperkeeper-server`. The binary is a thin `main` over the library
`trapperkeeper_server`, which parses three subcommands. `migrate` creates the
relational store's database on SQLite, Postgres or MariaDB when it is missing
and applies the migrations embedded in the binary. `serve` sets up an empty
database on its first start, refuses a database that does not match those
migrations, then binds the listen address and runs the HTTP server until a
signal stops it. Setting up a database, by serve or by migrate, creates the
superadmin and logs its one-time password. `reset-superadmin` gives the
superadmin, the user `admin` of the system tenant `$tk`, a new one-time
password, creating the superadmin when it is missing, and prints the
password. Before a
subcommand runs, the binary resolves its settings: the listen address, the log
level and the database URL. Each module directory that appears under `src/`
gets its own `AGENTS.md`, and this file indexes them.

**Map**:

| File                  | What                                                                                                                                                                                                                                          | When to read                                                                       |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `Cargo.toml`          | package and binary names; dependencies; lints                                                                                                                                                                                                 | changing a dependency or a lint                                                    |
| `build.rs`            | reruns the build when a migration file changes                                                                                                                                                                                                | why a new migration file is picked up                                              |
| `src/lib.rs`          | the entry point: parse, resolve, dispatch; what the library exports                                                                                                                                                                           | how a subcommand starts and how it fails; what tests can reach                     |
| `src/main.rs`         | starts the tokio runtime and calls the library                                                                                                                                                                                                | why the binary is so small                                                         |
| `src/accounts.rs`     | creating users with a generated password that must be changed                                                                                                                                                                                 | creating a user, what a new user must do first                                     |
| `src/auth/`           | authentication of people: passwords, signed tokens, login, the caller of a route; see its `AGENTS.md`                                                                                                                                         | passwords, tokens, the signing keys, logging in                                    |
| `src/cli.rs`          | the command line                                                                                                                                                                                                                              | adding a flag or a subcommand                                                      |
| `src/config.rs`       | the settings and how they are resolved                                                                                                                                                                                                        | adding a setting, precedence, environment variable names, the log level            |
| `src/http.rs`         | the router and its state, the `/api/v1` prefix, how the server is served and stopped                                                                                                                                                          | adding an endpoint, what a handler can reach, how shutdown works                   |
| `src/logging.rs`      | the subscriber: log format and level                                                                                                                                                                                                          | log format, log level, RUST_LOG                                                    |
| `src/names.rs`        | the tenant name and user name rules                                                                                                                                                                                                           | what a created name may hold, the `$` prefix                                       |
| `src/scope.rs`        | the rule for the tenant in a route's path                                                                                                                                                                                                     | who may call a tenant-scoped route                                                 |
| `src/store/`          | the relational store and its migrations; see its `AGENTS.md`                                                                                                                                                                                  | the database, migrations, tables                                                   |
| `src/superadmin.rs`   | the superadmin of `$tk`, reset from the console                                                                                                                                                                                               | the superadmin, recovering its password                                            |
| `src/tenants.rs`      | the superadmin's tenant routes                                                                                                                                                                                                                | creating and listing tenants                                                       |
| `src/users.rs`        | the admin's routes for a tenant's users                                                                                                                                                                                                       | adding a user route                                                                |
| `tests/cli.rs`        | end-to-end: runs the binary and checks its command line                                                                                                                                                                                       | how the binary behaves on the command line                                         |
| `tests/common/mod.rs` | test helpers: the backends a run tests, test databases, a pool on one of them with placeholders rewritten, the router served in-process for integration tests, the binary started as a process for end-to-end tests, run migrate, GET a route | writing a server test; testing on every backend; which kind of server a test needs |
| `tests/http.rs`       | integration tests of the endpoints on the router served in-process                                                                                                                                                                            | what the probes and the document promise                                           |
| `tests/login.rs`      | integration tests of login, refresh, logout, password change and me on the router served in-process: a scenario, a property over defective tokens, and a property over sequences of the routes against a model                                | what the routes hand out and refuse, which tokens pass, an unknown tenant at login |
| `tests/logs.rs`       | end-to-end: checks the log lines the binary writes under a pipe                                                                                                                                                                               | what a log line contains                                                           |
| `tests/settings.rs`   | end-to-end: the settings precedence as one property through the binary                                                                                                                                                                        | how precedence is tested                                                           |
| `tests/shutdown.rs`   | end-to-end test that SIGTERM stops the binary                                                                                                                                                                                                 | how the server stops                                                               |
| `tests/store.rs`      | end-to-end: runs migrate and serve against SQLite files                                                                                                                                                                                       | what migrate builds, which databases serve accepts, what the first serve sets up   |
| `tests/superadmin.rs` | end-to-end runs of reset-superadmin                                                                                                                                                                                                           | what reset-superadmin prints and stores                                            |
| `tests/tenants.rs`    | integration tests of the tenant routes and the role-against-route test                                                                                                                                                                        | who may call which route, creating and listing tenants                             |

**Invariants**:

- A setting is added in `config::Settings` with a default, and its flag in
  `cli::Overrides` with `skip_serializing_if`, nested the same way, so the two
  trees stay the same shape and an absent flag changes nothing. The signing
  keys are the exception: they are secrets with no flag, and serve refuses
  to start without `auth.signing_key`.
- A secret setting has a type that masks it when serialized or printed, as
  `config::DatabaseUrl` and `config::SigningKey` do. Its flag, where it has
  one, stays a plain value, because the flags layer is serialized into the
  merge, and its default holds no secret.
- Every management route sits under `/api/v1` and takes `Caller` or
  `AtLeast<R>`, except login and refresh, which take the credentials
  themselves. The probes and
  the API document stay at the root.
- A tenant-scoped route is `/api/v1/tenants/{tenant}/...`, and its caller is
  a member of that tenant at the route's minimum role, or the superadmin
  (owner, 2026-11-07). `scope::resolve` is that rule.
- The system tenant is not reachable under `/api/v1/tenants/{tenant}/...`;
  its one user, the superadmin, is managed from the console and its own
  account routes (`doc/adr/0028`).
- Every handler carries a `utoipa::path` annotation and enters the router
  through `routes!`; a handler added with `.route` is missing from the API
  document. A handler's doc comment is its operation summary in that document,
  so it is written for the API's readers.
- A user is created only through `accounts::create`, so every user gets a
  generated password that must be changed at the first login.
- A password reaches the console through `println` when the command's
  output is the password, and as one INFO event when the command sets up a
  database, so a piped log stream stays JSON lines.
- An integration test builds the server in the test process through the
  library and never starts a process. Starting the binary is reserved for
  end-to-end tests, which check what only a process shows: the exit status,
  the command line, the log output on stdout and signals (owner, 2026-11-04).
- A test never picks a free port, releases it and passes it on to a server.
  An in-process test keeps the listener it bound, and a process test starts
  serve on port 0 and reads the bound address from the "listening" line.

**Why**:

- Tenants of a hosted and of a self-hosted install share one database, and
  a login names its tenant (owner, 2026-11-06):
  `doc/adr/0003-tenants-share-one-database.md`.
- The superadmin is the user `admin` of the system tenant `$tk`, created by
  the first migration of an empty database and reset from the console
  (owner, 2026-11-06): `doc/adr/0028-system-tenant-and-first-run.md`.
- The package and the binary have different names; the reason sits at
  `[[bin]]` in `Cargo.toml`.
- clap parses the command line and figment merges the settings layers:
  `doc/adr/0012-clap-and-figment-for-the-command-line-and-configuration.md`.
- Secrets may come from the file or the environment, the command line warns,
  and credentials are never configuration (owner, 2026-11-01):
  `doc/adr/0013-where-secrets-may-come-from.md`. The database URL is the
  first secret.
- Everything is async wherever an async form exists (owner, 2026-11-01):
  `doc/adr/0014-async-everywhere.md`.
- The web UI is rendered on the server, no single-page application (owner,
  2026-11-01): `doc/adr/0015-server-side-rendering-no-spa.md`.
- axum with tower middleware serves HTTP, and utoipa generates the API
  document from the handlers (owner, 2026-11-01):
  `doc/adr/0016-axum-and-utoipa-for-http.md`.
- tracing writes the logs, JSON under a pipe and readable in a terminal, at a
  level setting that `RUST_LOG` overrides (owner, 2026-11-01):
  `doc/adr/0017-tracing-for-structured-logs.md`.
- sqlx runs hand-written SQL over SQLite, Postgres and MariaDB, with
  checksummed migrations per backend (owner, 2026-11-02):
  `doc/adr/0019-sqlx-with-hand-written-sql-over-three-backends.md`.
- An entity's `_id` is an internal time-ordered UUID, and its `_name` is
  unique, lowercase and what people use (owner, 2026-11-02):
  `doc/adr/0020-internal-ids-and-unique-names.md`.
- Every test runs on every CI platform, against the latest stable or LTS
  release of each database, and a platform skips only what has no build for
  it (owner, 2026-11-02):
  `doc/adr/0021-every-test-suite-runs-on-every-platform.md`.
- Tests run at the highest level that observes a behavior, by default.
  Integration tests build the server in the test process and start no
  process, and only end-to-end tests start the binary. Tests generate their
  data where it varies (owner, 2026-11-01, 2026-11-03, 2026-11-04):
  `doc/adr/0018-integration-tests-over-unit-tests.md`.
- Passwords are hashed with Argon2id at RFC 9106's second recommended
  option, limited to the CPU count (owner, 2026-11-05):
  `doc/adr/0022-argon2id-for-passwords.md`.
- Roles are rows seeded per tenant (owner, 2026-11-05):
  `doc/adr/0023-roles-are-rows-per-tenant.md`.
- Entities are deleted softly (owner, 2026-11-05):
  `doc/adr/0024-soft-deletes.md`.
- People log in for signed tokens the server does not store, revoked per user
  by a cutoff, with two hand-typed signing keys (owner, 2026-11-05):
  `doc/adr/0025-signed-tokens-and-the-revocation-cutoff.md`.
- Management routes live under `/api/v1` (owner, 2026-11-05):
  `doc/adr/0026-versioned-api-route-prefix.md`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo run -- --help` shows the command line. `cargo run -- serve` on
a missing database creates `trapperkeeper.db` in the working directory, sets
it up, logs the superadmin's one-time password, and starts the server on
`127.0.0.1:8080`; Ctrl-C stops it. A local serve
needs a signing key: `signing_key = "<any passphrase of 32 or more
characters>"` under `[auth]` in the file, or
`TRAPPERKEEPER_AUTH__SIGNING_KEY` in the environment. `cargo run --
reset-superadmin` prints a new one-time password for the superadmin.
The tests run against SQLite, and also against Postgres and MariaDB when
`TK_TEST_POSTGRES_URL` and `TK_TEST_MARIADB_URL` are set; see
`src/store/AGENTS.md`, How to. `RUST_LOG=trapperkeeper_server=debug cargo run -- serve` is a debugging run; piping
through `jq` reads the JSON lines.
