# 0032. Concurrent tests own their state

Date: 2026-11-09.

## Context

`cargo test` runs the tests of one binary on parallel threads, and the
server's integration tests run against live databases on SQLite, Postgres and
MariaDB (`0018`, `0021`). The two server backends are one server each, shared
by every test of the run, so two tests could read each other's rows unless
something keeps them apart.

## Decision

Tests run concurrently and are kept apart by what they own, never by
serializing them (owner, 2026-11-09).

Each test owns everything it touches. On SQLite its database is a file in a
scratch directory of its own, removed with the test. On Postgres and MariaDB
its database is created on the server the admin URL names, with a `CREATE
DATABASE` for a name made of the process id and a counter, and dropped when
the test ends. A test that serves the router binds a listener of its own on
port 0 and keeps it, and holds its own store, derived keys and tokens. No
test reads another test's rows.

The tokio runtime is shared by the whole test process, because a sqlx
connection belongs to the runtime that opened it. It is an executor and holds
no test's state.

A resource the process shares is either locked for the case or used by a
single test in its binary: the settings property changes the environment
inside figment's jail, which holds a global lock and restores the environment
after, and the log test installs the global tracing subscriber and is the only
test of its binary.

Tests that will run against a live TrapperKeeper server, such as the SDKs'
integration tests, are expected to follow the same rule at the level that
server offers, a tenant of its own per test (owner, 2026-11-09). The SDK
records settle it when those tests are written.

## Rejected alternatives

Running the tests one at a time, because the suite's time on the server
backends would grow with every test, and a serial suite hides a test that
depends on another's leftovers.

One database shared by every test, with per-test names or ids inside it,
because a dropped database leaves nothing behind, and the first-run scenarios
need a database that does not exist yet.

## Consequences

The admin URL of a server backend must belong to a user allowed to create and
drop databases. A test that is killed before its drop leaves a database named
`tk_test_*` behind, which the next run does not reuse. Postgres databases are
dropped by force, because a stopped server's connections may not have closed
yet. Every test pays a database creation per backend, which is why the store
property runs fewer cases than proptest's default.
