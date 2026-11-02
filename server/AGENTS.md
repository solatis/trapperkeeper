# server/

**What**: the single server binary, `trapperkeeper`, built from the package
`trapperkeeper-server`. It parses two subcommands. `migrate` creates the
relational store's database on SQLite, Postgres or MariaDB when it is missing and applies the migrations
embedded in the binary. `serve` refuses a database that does not match those
migrations, then binds the listen address and runs the HTTP server until a
signal stops it. Before a subcommand runs, the binary resolves
its settings: the listen address, the log level and the database URL. The APIs
arrive with the next M1 issues. Each module directory that appears under
`src/` gets its own `AGENTS.md`, and this file indexes them.

**Map**:

| File                  | What                                                                             | When to read                                                            |
| --------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `Cargo.toml`          | package and binary names; dependencies; lints                                    | changing a dependency or a lint                                         |
| `build.rs`            | reruns the build when a migration file changes                                   | why a new migration file is picked up                                   |
| `src/main.rs`         | the entry point: parse, resolve, dispatch                                        | how a subcommand starts and how it fails                                |
| `src/cli.rs`          | the command line                                                                 | adding a flag or a subcommand                                           |
| `src/config.rs`       | the settings and how they are resolved                                           | adding a setting, precedence, environment variable names, the log level |
| `src/http.rs`         | the router, how it is served and stopped                                         | adding an endpoint, how shutdown works                                  |
| `src/logging.rs`      | the subscriber: log format and level                                             | log format, log level, RUST_LOG                                         |
| `src/store/`          | the relational store and its migrations; see its `AGENTS.md`                     | the database, migrations, tables                                        |
| `tests/cli.rs`        | runs the binary and checks its command line                                      | how the binary behaves on the command line                              |
| `tests/common/mod.rs` | process helpers: scratch directories, spawn the server, run migrate, GET a route | writing a test that runs the binary                                     |
| `tests/http.rs`       | starts the binary and probes its endpoints                                       | what the probes and the document promise                                |
| `tests/logs.rs`       | runs the binary piped and checks the log lines                                   | what a log line contains                                                |
| `tests/settings.rs`   | the settings precedence as one property through the binary                       | how precedence is tested                                                |
| `tests/store.rs`      | runs migrate and serve against SQLite files                                      | what migrate builds, which databases serve accepts                      |

**Invariants**:

- A setting is added in `config::Settings` with a default, and its flag in
  `cli::Overrides` with `skip_serializing_if`, nested the same way, so the two
  trees stay the same shape and an absent flag changes nothing.
- Every handler carries a `utoipa::path` annotation and enters the router
  through `routes!`; a handler added with `.route` is missing from the API
  document. A handler's doc comment is its operation summary in that document,
  so it is written for the API's readers.

**Why**:

- The package and the binary have different names; the reason sits at
  `[[bin]]` in `Cargo.toml`.
- clap parses the command line and figment merges the settings layers:
  `doc/adr/0012-clap-and-figment-for-the-command-line-and-configuration.md`.
- Secrets may come from the file or the environment, the command line warns,
  and credentials are never configuration (owner, 2026-11-01):
  `doc/adr/0013-where-secrets-may-come-from.md`. No secret exists yet.
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
- Tests drive the binary as a process, one test per behavior, and generate
  their data where it varies (owner, 2026-11-01):
  `doc/adr/0018-integration-tests-over-unit-tests.md`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo run -- --help` shows the command line. `cargo run -- migrate`
creates `trapperkeeper.db` in the working directory, and `cargo run -- serve`
then starts the server on `127.0.0.1:8080`; Ctrl-C stops it.
`RUST_LOG=trapperkeeper=debug cargo run -- serve` is a debugging run; piping
through `jq` reads the JSON lines.
