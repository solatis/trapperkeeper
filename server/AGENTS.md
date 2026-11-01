# server/

**What**: the single server binary, `trapperkeeper`, built from the package
`trapperkeeper-server`. It parses two subcommands, `serve` and `migrate`; both
are placeholders that report "not implemented yet" until issue #4 and the
store issue fill them in. Before a subcommand runs, the binary resolves its
settings; one setting exists, the listen address. The store and the HTTP APIs
arrive with the next M1 issues. Each module directory that appears under
`src/` gets its own `AGENTS.md`, and this file indexes them.

**Map**:

| File            | What                                               | When to read                               |
| --------------- | -------------------------------------------------- | ------------------------------------------ |
| `Cargo.toml`    | package and binary names; dependencies; lints      | changing a dependency or a lint            |
| `src/main.rs`   | the entry point: parse, dispatch, the placeholders | how a subcommand starts and how it fails   |
| `src/cli.rs`    | the command line                                   | adding a flag or a subcommand              |
| `src/config.rs` | the settings and how they are resolved             | adding a setting, the defaults             |
| `tests/cli.rs`  | runs the binary and checks its command line        | how the binary behaves on the command line |

**Why**:

- The package and the binary have different names; the reason sits at
  `[[bin]]` in `Cargo.toml`.
- clap parses the command line and figment merges the settings layers:
  `doc/adr/0012-clap-and-figment-for-the-command-line-and-configuration.md`.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. `cargo run -- --help` shows the command line.
