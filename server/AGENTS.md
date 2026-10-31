# server/

**What**: the single server binary, `trapperkeeper`, built from the package
`trapperkeeper-server`. Today it prints its name and version; the subcommands,
configuration, store and HTTP APIs arrive with the next M1 issues. Each module
directory that appears under `src/` gets its own `AGENTS.md`, and this file
indexes them.

**Map**:

| File               | What                                                         | When to read                          |
| ------------------ | ------------------------------------------------------------ | ------------------------------------- |
| `Cargo.toml`       | package and binary names; lints inherited from the workspace | changing a dependency or a lint       |
| `src/main.rs`      | the entry point                                              | always short to read                  |
| `tests/version.rs` | runs the binary and checks its version line                  | adding the first test of a subcommand |

**Why**:

- Package `trapperkeeper-server`, binary `trapperkeeper`: the milestones name
  the binary (`trapperkeeper eval` in M3), and the package keeps the `-server`
  suffix so the bare crate name stays free for the Rust sensor library.

**How to**: build, lint and test with the commands in the root `AGENTS.md`,
How to. Nothing is crate-specific yet.
