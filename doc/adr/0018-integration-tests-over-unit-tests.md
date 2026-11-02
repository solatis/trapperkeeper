# 0018. Integration tests over unit tests, properties over hand-picked data

Date: 2026-11-02.

## Context

The first settings code arrived with eight small unit tests of the loader, one
hand-written case each, beside process tests of the binary. The owner reviewed
them on 2026-11-01 and stated how this project tests. The owner refined the
decision on 2026-11-02, while the store tests were planned.

## Decision

Tests are designed, architected and commented with the same care as the code
they test, because they are maintained code and a risk of technical debt. A
test covers a large surface. A behavior is tested once, at the highest level
that observes it: an integration test that drives the binary or a component
through its real interface, because it is the more reliable signal that the
behavior works. A unit test that duplicates an integration test is removed.
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
does not own.

## Consequences

A behavior may go untested until something observes it from outside: the
settings precedence waits for the server to log its resolved settings. Test
binaries start processes. proptest is a development dependency. sqlx's own
refusals of a database, such as a migration that failed half way, have no test
here, and a sqlx regression shows only through the store property. The 503
answer of readiness has no test, because a test would have to break a live
store.
