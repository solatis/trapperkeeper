# 0006. Conformance fixtures are the evaluation spec

Date: 2026-09-21. Status: accepted.

## Context

The retired first attempt specified evaluation semantics in prose, and the
prose contradicted itself.

## Decision

Language-neutral fixtures (rule, record, expected outcome) are the
specification of evaluation. They are reviewed once and enforced in every SDK.
Field types are `int`, `float`, `string`, `boolean` and `any`.

## Rejected alternatives

A prose specification, because it contradicted itself.

## Consequences

Every SDK must pass the fixtures, and a question about semantics is answered
by adding a fixture. The conformance suite cites this record.
