# 0018. Integration tests over unit tests, properties over hand-picked data

Date: 2026-11-02.

## Context

The first settings code arrived with eight small unit tests of the loader, one
hand-written case each, beside process tests of the binary. The owner reviewed
them on 2026-11-01 and stated how this project tests. The owner refined the
decision on 2026-11-02, while the store tests were planned, and again on
2026-11-03 and 2026-11-04, while the endpoint tests moved into the test
process.

## Decision

Tests are designed, architected and commented with the same care as the code
they test, because they are maintained code and a risk of technical debt. A
test covers a large surface. By default a behavior is tested once, at the
highest level that observes it, because that is the more reliable signal that
the behavior works. The test's author picks the level with judgment (owner,
2026-11-03). A unit test that duplicates an integration test is removed.

An integration test drives a component through its real interface. A test
that builds the server in its own process, through the server's library, is
an integration test, and test helpers that do so are fine. An integration
test never starts a process. Starting the binary is reserved for end-to-end
tests, which check what only a process shows: the exit status, the command
line, the log output on stdout and signals. A test that starts the binary to
check a route is an end-to-end test (owner, 2026-11-03 and 2026-11-04).

Where the data varies, a test states a property and generates the data
(proptest) instead of listing cases by hand. Several small tests of one
component are combined into a larger integration test or a property, as the
store tests are (owner, 2026-11-02).

Tests cover this repository's code. The behavior of a library it uses is out
of scope, so no test checks what the library already guarantees (owner,
2026-11-02). Code is not bent for a test: a path that only test-only code
could reach stays untested (owner, 2026-11-02).

## Rejected alternatives

Many small unit tests for fast feedback, because fast feedback is not a goal
here and the suite becomes debt. A unit test and an integration test of the
same behavior, because the unit test then adds maintenance and no signal. A
test per refusal that a library makes, because it tests code this repository
does not own. Integration tests that prepare their database with the `migrate`
binary, because a test that starts a process is an end-to-end test.

## Consequences

A behavior may go untested until something observes it from outside: the
settings precedence waits for the server to log its resolved settings. Test
binaries start processes. The server crate has a library target, so tests
reach the router and the store. proptest is a development dependency. sqlx's own
refusals of a database, such as a migration that failed half way, have no test
here, and a sqlx regression shows only through the store property. The 503
answer of readiness has no test, because a test would have to break a live
store.
