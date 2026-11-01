# doc/adr/

Architecture decision records: one file per decision, saying what was decided,
why, what lost and what it costs. A record is written once and never edited; a
change of mind is a new record that names the one it supersedes. Records are
numbered in the order the decisions were made, and the number is how code
comments and `AGENTS.md` files cite them.

Each record has the same parts: `Date` and `Status` (accepted or superseded by
a later number) on the first line, then **Context**, **Decision**, **Rejected
alternatives** and **Consequences**. A part with nothing recorded says so
rather than being dropped.

| Record                                                 | Decision                                                               |
| ------------------------------------------------------ | ---------------------------------------------------------------------- |
| `0001-management-api-is-the-single-write-path.md`      | every write that is not an event goes through one API                  |
| `0002-ai-agent-is-an-external-client.md`               | the agent is a client with its own principal and manages rules only    |
| `0003-tenant-aware-model-single-tenant-operation.md`   | everything carries a tenant id, one default tenant ships               |
| `0004-events-embed-rule-snapshots.md`                  | an event carries the rule as it was evaluated                          |
| `0005-one-vocabulary-for-failure-behavior.md`          | pass-through, halt, cached per sensor; skip, match, fail per condition |
| `0006-conformance-fixtures-are-the-evaluation-spec.md` | fixtures, not prose, define evaluation                                 |
| `0007-rust-for-the-server-side.md`                     | the server and its APIs are written in Rust                            |
| `0008-monorepo.md`                                     | all components live in this repository                                 |
| `0009-latest-stable-rust-only.md`                      | no minimum Rust version below the current stable                       |
| `0010-one-top-level-directory-per-component.md`        | `server/`, `engine/`, `sdk/<lang>/`, `web/`, `conformance/`            |
| `0011-clippy-pedantic-denied-everywhere.md`            | clippy pedantic and rustc warnings fail every build, local or CI       |
