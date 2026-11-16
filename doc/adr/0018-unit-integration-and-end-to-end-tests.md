# 0018. Unit, integration and end-to-end tests

Date: 2026-11-09.

## Context

The server stores records and reads them back through a database and a
router. The rule engine is pure logic over records. The two need different
kinds of test. This record fixed the vocabulary before the engine's tests were
written (owner, 2026-11-09).

Tests are maintained code and a risk of technical debt. For that reason the
project decides which kinds of test exist and what each kind covers.

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

The test suite avoids covering one behavior with both a unit test and an
integration test. The unit tests of a component are its exhaustive tests. An
integration test of a component that uses another component checks how the
two work together, and it does not test the surface of the component it uses
(owner, 2026-11-09).

Property, behavioral and data-driven tests are preferred. A complicated or
important edge case may have a dedicated test. Tests are designed and
commented with the same care as the code they test (owner, 2026-11-09).

Tests cover the code of this repository. No test checks what a library
already guarantees. The authors do not change code to make it testable. A path
that only test-only code could reach stays untested (owner, 2026-11-02).

What only a process shows stays untested. An end-to-end layer may come later,
and the integration tests of the SDKs, which run the real binaries, are a
likely place for it (owner, 2026-11-09).

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

The tests drive the server crate as a library. The crate exports what the
integration tests need, and `server/src/lib.rs` says which items those are.

No test covers exit codes, the usage text, terminal detection (JSON under a
pipe against the compact format on a terminal), the place of the signing-key
check before the database, the signal handlers, sqlx's own refusals of a
database such as a half-applied migration, or the 503 answer of readiness. A
test of the 503 would have to break a live store.

proptest and figment's `test` feature are development dependencies.

The tests of an SDK are expected to run against a real server. Each SDK
records this choice for itself.
