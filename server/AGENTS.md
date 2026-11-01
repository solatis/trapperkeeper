# server/

**What**: the single server binary, `trapperkeeper`, built from the package
`trapperkeeper-server`. It parses two subcommands. `serve` binds the listen
address and runs the HTTP server until a signal stops it; `migrate` is a
placeholder that reports "not implemented yet" until the store issue fills it
in. Before a subcommand runs, the binary resolves its settings; one setting
exists, the listen address. The store and the APIs arrive with the next M1
issues. Each module directory that appears under
`src/` gets its own `AGENTS.md`, and this file indexes them.

**Map**:

| File            | What                                              | When to read                                             |
| --------------- | ------------------------------------------------- | -------------------------------------------------------- |
| `Cargo.toml`    | package and binary names; dependencies; lints     | changing a dependency or a lint                          |
| `src/main.rs`   | the entry point: parse, dispatch, the placeholder | how a subcommand starts and how it fails                 |
| `src/cli.rs`    | the command line                                  | adding a flag or a subcommand                            |
| `src/config.rs` | the settings and how they are resolved            | adding a setting, precedence, environment variable names |
| `src/http.rs`   | the router, how it is served and stopped          | adding an endpoint, how shutdown works                   |
| `tests/cli.rs`  | runs the binary and checks its command line       | how the binary behaves on the command line               |

**Invariants**:

- A setting is added in `config::Settings` with a default, and its flag in
  `cli::Overrides` with `skip_serializing_if`, so the two trees stay the same
  shape and an absent flag changes nothing.

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
- Tests drive the binary as a process, one test per behavior, and generate
  their data where it varies (owner, 2026-11-01):
  `doc/adr/0018-integration-tests-over-unit-tests.md`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo run -- --help` shows the command line; `cargo run -- serve`
starts the server on `127.0.0.1:8080`, and Ctrl-C stops it.
