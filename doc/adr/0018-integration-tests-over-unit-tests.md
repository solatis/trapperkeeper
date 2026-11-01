# 0018. Integration tests over unit tests, properties over hand-picked data

Date: 2026-11-01.

## Context

The first settings code arrived with eight small unit tests of the loader, one
hand-written case each, beside process tests of the binary. The owner reviewed
them on 2026-11-01 and stated how this project tests.

## Decision

Tests are designed, architected and commented with the same care as the code
they test, because they are maintained code and a risk of technical debt. A
test covers a large surface. A behavior is tested once, at the highest level
that observes it: an integration test that drives the binary or a component
through its real interface, because it is the more reliable signal that the
behavior works. A unit test that duplicates an integration test is removed.
Where the data varies, a test states a property and generates the data
(proptest) instead of listing cases by hand.

## Rejected alternatives

Many small unit tests for fast feedback, because fast feedback is not a goal
here and the suite becomes debt. A unit test and an integration test of the
same behavior, because the unit test then adds maintenance and no signal.

## Consequences

A behavior may go untested until something observes it from outside: the
settings precedence waits for the server to log its resolved settings. Test
binaries start processes. proptest is a development dependency.
