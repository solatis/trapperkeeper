# doc/adr/

Architecture decision records: one file per decision, saying what was decided,
why, what lost and what it costs. A record describes the decision as it stands
today. When a decision changes, its record is rewritten in place, as if the
older decision had never been made; the repository keeps no historical trail
of decisions (owner, 2026-11-01). Records are numbered in the order the
decisions were first made, and the number is how code comments and `AGENTS.md`
files cite them.

Each record has the same parts: `Date`, the day the decision as written was
made, then **Context**, **Decision**, **Rejected alternatives** and
**Consequences**. A part with nothing recorded says so rather than being
dropped.

| Record                                                            | Decision                                                                                        |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `0001-management-api-is-the-single-write-path.md`                 | every write that is not an event goes through one API                                           |
| `0002-ai-agent-is-an-external-client.md`                          | the agent is a client with its own principal and manages rules only                             |
| `0003-tenant-aware-model-single-tenant-operation.md`              | everything carries a tenant id, one default tenant ships                                        |
| `0004-events-embed-rule-snapshots.md`                             | an event carries the rule as it was evaluated                                                   |
| `0005-one-vocabulary-for-failure-behavior.md`                     | pass-through, halt, cached per sensor; skip, match, fail per condition                          |
| `0006-conformance-fixtures-are-the-evaluation-spec.md`            | fixtures, not prose, define evaluation                                                          |
| `0007-rust-for-the-server-side.md`                                | the server and its APIs are written in Rust                                                     |
| `0008-monorepo.md`                                                | all components live in this repository                                                          |
| `0009-latest-stable-rust-only.md`                                 | no minimum Rust version below the current stable                                                |
| `0010-one-top-level-directory-per-component.md`                   | `server/`, `engine/`, `sdk/<lang>/`, `web/`, `conformance/`                                     |
| `0011-clippy-pedantic-denied-everywhere.md`                       | clippy pedantic and rustc warnings fail every build, local or CI                                |
| `0012-clap-and-figment-for-the-command-line-and-configuration.md` | clap parses the command line, figment merges the settings layers                                |
| `0013-where-secrets-may-come-from.md`                             | secrets in the file or the environment, the command line warns, credentials never               |
| `0014-async-everywhere.md`                                        | async wherever an async form exists, on tokio                                                   |
| `0015-server-side-rendering-no-spa.md`                            | the web UI is rendered on the server, JavaScript only for small widgets                         |
| `0016-axum-and-utoipa-for-http.md`                                | axum with tower middleware; utoipa generates the API document from the handlers                 |
| `0017-tracing-for-structured-logs.md`                             | tracing; JSON under a pipe, readable in a terminal; a log level setting that RUST_LOG overrides |
| `0018-integration-tests-over-unit-tests.md`                       | one test per behavior at the highest observing level; generated data over hand-picked cases     |
| `0019-sqlx-with-hand-written-sql-over-three-backends.md`          | sqlx, hand-written SQL in one subset, checksummed migrations per backend                        |
| `0020-internal-ids-and-unique-names.md`                           | `_id` is an internal time-ordered UUID; `_name` is unique and what people use                   |
| `0021-every-test-suite-runs-on-every-platform.md`                 | every test on every CI platform; latest stable or LTS dependencies; skip only without a build   |
| `0022-argon2id-for-passwords.md`                                  | Argon2id at 64 MiB, 3 passes, 4 lanes, as PHC strings; hashes limited to the CPU count          |
