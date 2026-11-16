# server/

**What**: `server/` holds the server binary `trapperkeeper`, and the package
`trapperkeeper-server` builds it. The binary is a thin `main` over the library
`trapperkeeper_server`, which parses three subcommands. `migrate` creates the
relational store's database on SQLite, Postgres or MariaDB when it is missing
and applies the migrations embedded in the binary. `serve` sets up an empty
database on its first start, refuses a database that does not match those
migrations, then binds the listen address and runs the HTTP server until a
signal stops it. Setting up a database, by serve or by migrate, creates the
superadmin and logs its one-time password. `reset-superadmin` gives the
superadmin, the user `admin` of the system tenant `$tk`, a new one-time
password, creating the superadmin when it is missing, and prints the
password. The superadmin creates tenants and mints a session for any user
through the management routes. Before a subcommand runs, the binary resolves
its settings: the listen address, the readiness timeout, the log level, the
database URL and the two signing keys. Each module directory that appears
under `src/` gets its own `AGENTS.md`, and this file indexes them.

**Map**:

| File                  | What                                                                                                                                                                                                                            | When to read                                                                                                                                         |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`          | package and binary names; dependencies; lints                                                                                                                                                                                   | changing a dependency or a lint                                                                                                                      |
| `build.rs`            | reruns the build when a migration file changes                                                                                                                                                                                  | why a new migration file is picked up                                                                                                                |
| `src/lib.rs`          | the entry point: parse, resolve, dispatch; what the library exports                                                                                                                                                             | how a subcommand starts and how it fails; what tests can reach                                                                                       |
| `src/main.rs`         | starts the tokio runtime and calls the library                                                                                                                                                                                  | why the binary is so small                                                                                                                           |
| `src/accounts.rs`     | creating users with a generated password that must be changed, and resetting a user's password to a new generated one                                                                                                           | creating a user, what a new user must do first, resetting a password                                                                                 |
| `src/audit.rs`        | the middleware that records changing requests, the identity cell, and the admin's read route                                                                                                                                    | what the trail records, how a handler names the caller and the tenant acted on, paging the trail                                                     |
| `src/auth/`           | authentication of people: passwords, signed tokens, login, the caller of a route, the role ladder; see its `AGENTS.md`                                                                                                          | passwords, tokens, the signing keys, logging in, the role ladder, which role a route needs                                                           |
| `src/cli.rs`          | the command line                                                                                                                                                                                                                | adding a flag or a subcommand                                                                                                                        |
| `src/config.rs`       | the settings and how they are resolved                                                                                                                                                                                          | adding a setting, precedence, environment variable names, the log level, which settings are secret and how they are masked, the signing-key settings |
| `src/http.rs`         | the router and its state, the `/api/v1` prefix, the probes, the API document at `/openapi.json`, how the server is served and stopped                                                                                           | adding an endpoint, what a handler can reach, how shutdown works, the probes, the API document                                                       |
| `src/logging.rs`      | the subscriber: log format and level; the JSON subscriber a test installs                                                                                                                                                       | log format, log level, RUST_LOG                                                                                                                      |
| `src/names.rs`        | the tenant name and user name rules                                                                                                                                                                                             | what a created name may hold, the `$` prefix                                                                                                         |
| `src/scope.rs`        | the rule for the tenant in a route's path                                                                                                                                                                                       | who may call a tenant-scoped route                                                                                                                   |
| `src/sessions.rs`     | the superadmin's route that mints a session for a user                                                                                                                                                                          | acting as a user                                                                                                                                     |
| `src/setup.rs`        | opening, checking and setting up the database for the three commands                                                                                                                                                            | what serve, migrate and reset-superadmin accept                                                                                                      |
| `src/store/`          | the relational store and its migrations; see its `AGENTS.md`                                                                                                                                                                    | the database, migrations, tables                                                                                                                     |
| `src/superadmin.rs`   | the superadmin of `$tk`: created when a set-up finds it missing, reset from the console                                                                                                                                         | the superadmin, recovering its password, how a set-up creates the superadmin                                                                         |
| `src/tenants.rs`      | the superadmin's tenant routes                                                                                                                                                                                                  | creating and listing tenants                                                                                                                         |
| `src/users.rs`        | the routes for a tenant's users, which need admin, except delete, which a user may also call on itself                                                                                                                          | adding a user route, the last-admin rule, who may delete whom, resetting a user's password                                                           |
| `tests/audit.rs`      | integration tests of the audit trail                                                                                                                                                                                            | which requests leave rows, what a row holds, paging the trail                                                                                        |
| `tests/common/mod.rs` | test helpers: the backends a run tests, test databases, a pool on one of them with placeholders rewritten, a database set up the way migrate does, the router served in-process, GET a route, log in and call a route as a user | writing a server test; testing on every backend                                                                                                      |
| `tests/http.rs`       | integration tests of the endpoints on the router served in-process                                                                                                                                                              | what the probes and the document promise                                                                                                             |
| `tests/login.rs`      | integration tests of login, refresh, logout, password change and me on the router served in-process: a scenario, a property over defective tokens, and a property over sequences of the routes against a model                  | what the routes hand out and refuse, which tokens pass, an unknown tenant at login                                                                   |
| `tests/logs.rs`       | the JSON log lines on the router served in-process                                                                                                                                                                              | what a log line contains                                                                                                                             |
| `tests/schema.rs`     | what the tables that migrate builds refuse, on every backend                                                                                                                                                                    | which rows a table refuses                                                                                                                           |
| `tests/sessions.rs`   | integration tests of the mint route                                                                                                                                                                                             | what a minted session can do, what the mint refuses                                                                                                  |
| `tests/settings.rs`   | the settings precedence as one property through the loader, in figment's jail; the report of a secret flag; the refusal of a missing configuration file                                                                         | how precedence is tested, a secret flag, a missing config file                                                                                       |
| `tests/store.rs`      | what migrate builds, which databases serve accepts and what the first serve sets up, through the setup functions, on every backend                                                                                              | what migrate builds, which databases serve accepts, what the first serve sets up                                                                     |
| `tests/superadmin.rs` | reset-superadmin through the library, on every backend                                                                                                                                                                          | what reset-superadmin returns and stores                                                                                                             |
| `tests/tenants.rs`    | integration tests of the tenant routes and the role-against-route test                                                                                                                                                          | who may call which route, creating and listing tenants                                                                                               |
| `tests/users.rs`      | integration tests of the user routes on the router served in-process                                                                                                                                                            | managing a tenant's users, the last-admin rule, the tenant scope                                                                                     |

**Invariants**:

- A new setting gets a field with a default in `config::Settings`. Its flag
  gets a field in `cli::Overrides`, nested the same way, with
  `skip_serializing_if`. The two trees then have the same shape, and an absent
  flag changes nothing. The signing keys and `readiness_timeout_seconds` have
  no flag. serve refuses to start without `auth.signing_key`.
- A secret setting has a type that masks it when serialized or printed, as
  `config::DatabaseUrl` and `config::SigningKey` do. A secret's flag, where it
  has one, is a plain value, because the flags layer is serialized into the
  merge and a masked value would replace the real one. A secret's default
  holds no secret.
- Every management route sits under `/api/v1` and takes `Caller` or
  `AtLeast<R>`, except login and refresh, which take the credentials
  themselves. The probes and the API document stay at the root.
- A tenant-scoped route is `/api/v1/tenants/{tenant}/...`, and its caller is
  a member of that tenant at the route's minimum role, or the superadmin
  (owner, 2026-11-07). `scope::tenant` applies that rule, and every
  tenant-scoped route calls it.
- The system tenant is not reachable under `/api/v1/tenants/{tenant}/...`.
  Its one user, the superadmin, is managed from the console and its own
  account routes (`doc/adr/0028`).
- A tenant keeps at least one live user with the role `admin`. The user
  routes answer 409 to a delete or a demotion that would leave none, also
  when the caller is that user (owner, 2026-11-06, 2026-11-07).
- No route gives the role `superadmin`, so the superadmin is created only by
  the server (`doc/adr/0028`).
- Every non-GET request under `/api/v1` that matched a route leaves one
  audit row, written after the response and outside the handler's
  transaction.
- Every handler carries a `utoipa::path` annotation and enters the router
  through `routes!`. A handler added with `.route` is missing from the API
  document, and the route that serves `/openapi.json` is added that way, so
  that the document does not list itself. A handler's doc comment is its
  operation summary in that document, so it is written for the API's readers.
- A user is created only through `accounts::create`, so every user gets a
  generated password that must be changed at the first login.
- Every test runs in the test process and starts none. The behaviors that
  only a process shows stay untested: the exit status, the usage text, the
  terminal detection, the place of the signing-key check before the
  database, and the signal handlers (owner, 2026-11-09).
- Tests run concurrently and are kept apart by what they own, never by
  serializing them (`doc/adr/0032`). Each test reserves a database of its
  own on every backend, binds a listener of its own on port 0 and keeps it,
  and reads no other test's rows. A resource that the process shares is
  locked for the case or used by a single test. The settings property
  changes the environment inside figment's jail. The log test installs the
  global subscriber and is the only test of its binary.

**Why**:

- The management API, whose code is in the server, is the single write path
  for configuration (owner, 2026-11-11):
  `doc/adr/0001-management-api-is-the-single-write-path.md`.
- Tenants of a hosted and of a self-hosted install share one database, and
  a login names its tenant (owner, 2026-11-06):
  `doc/adr/0003-tenants-share-one-database.md`.
- The server is written in Rust: `doc/adr/0007-rust-for-the-server-side.md`.
- The superadmin is the user `admin` of the system tenant `$tk`. The command
  that first migrates an empty database creates it after the migrations,
  which seed `$tk` and its role, and the console resets it (owner,
  2026-11-06): `doc/adr/0028-system-tenant-and-first-run.md`.
- The package and the binary have different names. The comment at `[[bin]]`
  in `Cargo.toml` says why.
- clap parses the command line and figment merges the settings layers:
  `doc/adr/0012-clap-and-figment-for-the-command-line-and-configuration.md`.
- A secret setting may come from the file or the environment. The server
  accepts a secret on the command line and logs a warning. User and
  administrator credentials are never configuration (owner, 2026-11-01):
  `doc/adr/0013-where-secrets-may-come-from.md`.
- Everything is async wherever an async form exists (owner, 2026-11-01):
  `doc/adr/0014-async-everywhere.md`.
- The server renders the web UI, and the project rejected a single-page
  application (owner, 2026-11-01):
  `doc/adr/0015-server-side-rendering-no-spa.md`.
- axum with tower middleware serves HTTP, and utoipa generates the API
  document from the handlers (owner, 2026-11-01):
  `doc/adr/0016-axum-and-utoipa-for-http.md`.
- tracing writes the logs, JSON under a pipe and readable in a terminal, at a
  level setting that `RUST_LOG` overrides (owner, 2026-11-01):
  `doc/adr/0017-tracing-for-structured-logs.md`.
- Every test runs on every CI platform, against the latest stable or LTS
  release of each database, and a platform skips only what has no build for
  it (owner, 2026-11-02):
  `doc/adr/0021-every-test-suite-runs-on-every-platform.md`.
- The tests are of three kinds, unit, integration and end-to-end. Only an
  end-to-end test starts a process, and the repository has none. No test
  mocks a subsystem without the owner's approval. Property tests are
  preferred (owner, 2026-11-01 to 2026-11-09):
  `doc/adr/0018-unit-integration-and-end-to-end-tests.md`.
- People log in for signed tokens the server does not store, revoked per user
  by a cutoff, with two hand-typed signing keys (owner, 2026-11-05):
  `doc/adr/0025-signed-tokens-and-the-revocation-cutoff.md`.
- Management routes live under `/api/v1` (owner, 2026-11-05):
  `doc/adr/0026-versioned-api-route-prefix.md`.
- The superadmin acts as a user through ordinary tokens minted for that
  user (owner, 2026-11-07): `doc/adr/0030-minted-sessions.md`.
- One middleware records the audit trail, and it stores the peer address and
  the forwarded header as received (owner, 2026-11-06, 2026-11-07):
  `doc/adr/0031-audit-trail.md`.
- Tests run concurrently and own their state, a database per test on every
  backend (owner, 2026-11-09):
  `doc/adr/0032-concurrent-tests-own-their-state.md`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo run -- --help` shows the command line. A local serve needs a
signing key: `signing_key = "<any passphrase of 32 or more characters>"`
under `[auth]` in the TOML file that `--config` names, or
`TRAPPERKEEPER_AUTH__SIGNING_KEY` in the environment. With the key,
`cargo run -- serve` on a missing database creates `trapperkeeper.db` in the
working directory, sets it up, logs the superadmin's one-time password, and
starts the server on `127.0.0.1:8080`. Ctrl-C stops it.
`cargo run -- reset-superadmin` prints a new one-time
password for the superadmin. The tests run against SQLite, and also against
Postgres and MariaDB when `TK_TEST_POSTGRES_URL` and `TK_TEST_MARIADB_URL`
are set, as `src/store/AGENTS.md`, How to, says.
`RUST_LOG=trapperkeeper_server=debug cargo run -- serve` runs the server with
the debug events of this crate, because `RUST_LOG` replaces the level
setting. `jq` makes the JSON lines readable when the output is piped into it.
