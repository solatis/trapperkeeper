# 0006. Conformance fixtures are the evaluation spec

Date: 2026-11-13.

## Context

The retired first attempt specified evaluation semantics in prose, and the
prose contradicted itself.

## Decision

Language-neutral fixtures are the specification of evaluation. They are
reviewed once and enforced in every SDK.

A fixture file covers one subject. An entry is a rule with many records and
the expected outcome of each (owner, 2026-11-11). An expected outcome is one
of three: matched, not matched, or not evaluated with its cause (`adr/0005`).

The runner runs each entry as single records and as one batch, and both give
the same outcomes (`adr/0037`).

The runner checks that each rule validates, refuses a property it does not
know, fails on an empty directory, and reports every mismatch when that stays
simple (owner, 2026-11-12).

There are no fixtures for validation, because no SDK validates a rule
(`adr/0033`).

The values of a record are ints, floats, text, booleans and lists. A rule
declares no field types (`adr/0041`).

## Rejected alternatives

A prose specification, because it contradicted itself.

One file per case, because the same rule would repeat in dozens of files.

Entries that hold a bare condition which the runner wraps into a rule,
because such an entry cannot express a whole rule.

Fixtures for validation, because nothing would run them.

## Consequences

Every SDK must pass the fixtures, and a question about semantics is answered
by adding a fixture. An expectation can differ by source where a source says
what "no value" is (`adr/0036`). The file format is fixed when the first
fixtures are written. The conformance suite cites this record.
