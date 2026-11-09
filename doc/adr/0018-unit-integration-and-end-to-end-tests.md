# 0018. Unit, integration and end-to-end tests

Date: 2026-11-09.

## Context

The server stores and reads back through a database and a router, and the
rule engine to come is pure logic over records. The two need different tests,
and this record fixes the vocabulary before the engine's tests are written
(owner, 2026-11-09).

Tests are maintained code and a risk of technical debt, so which kinds exist
and what each covers is a decision.

## Decision

There are three kinds of test.

A unit test exercises one piece of logic in the test process, with no outside
system. It is for a complex, self-contained component, such as a parser or an
evaluator, and a component of that kind gets exhaustive unit tests.

An integration test drives a component through its real interface with its
real dependencies, in the test process, against live systems. A route that
stores a record and reads it back through the real router and the real
database is an integration test.

An end-to-end test drives the built binary from shell commands against a live
install. It is the only kind that starts a process. The repository has none.

`cargo test --workspace` runs the unit and integration tests.

No test replaces a subsystem with a mock or a fake unless the owner approved
that one case through a question and its answer. Overriding one function to
force an error path in one integration test is allowed where nothing else
reaches the path (owner, 2026-11-09).

A behavior may be covered by a unit test and an integration test, but that
overlap is avoided: a component's unit tests are its exhaustive ones, and an
integration test of a component that uses it checks the integration, not the
used component's surface (owner, 2026-11-09).

Property, behavioral and data-driven tests are preferred. A complicated or
important edge case may have a dedicated test. Tests are designed and
commented with the same care as the code they test (owner, 2026-11-09).

Tests cover this repository's code. No test checks what a library already
guarantees, and code is not bent for a test: a path that only test-only code
could reach stays untested (owner, 2026-11-02).

What only a process shows is untested at this point: the exit status, clap's
usage text, JSON under a pipe against the compact format on a terminal, the
signing-key check running before the database is touched, and the signal
handlers. An end-to-end layer may come later, and the SDKs' integration tests,
which run the real binaries, are a likely place for it (owner, 2026-11-09).

## Rejected alternatives

Many small unit tests for fast feedback, because fast feedback is not a goal
here and the suite becomes debt.

Mocks of subsystems at the author's discretion, because a test that passes
against a fake says nothing about the real thing. An absolute ban on mocks,
because the owner does not want extreme rules.

Tests that start the binary under `cargo test`, because they test the process
edge, which is not worth its maintenance.

An end-to-end layer now, because nothing it would show is worth a CI step
today.

## Consequences

The server crate is a library the tests drive: the commands' open and set-up
of the database, the settings loader, the flag overrides and the JSON
subscriber are exported for them.

Untested: exit codes, usage text, terminal detection, the signing-key check's
place before the database, the signal handlers, sqlx's own refusals of a
database, such as a migration that failed half way, and the 503 answer of
readiness, because a test would have to break a live store.

proptest and figment's `test` feature are development dependencies.

An SDK's tests are expected to run against a real server; the choice is
recorded per SDK.
