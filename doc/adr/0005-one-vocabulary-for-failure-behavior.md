# 0005. One vocabulary for failure behavior

Date: 2026-09-21.

## Context

The retired first attempt had three vocabularies for what happens when
something goes wrong.

## Decision

Per sensor, when the server is unreachable: `pass-through` (default), `halt`,
`cached`. Per condition, on a missing field and on a coercion failure: `skip`
(default), `match`, `fail`. Null is missing; an uncoercible value is a coercion
failure and never missing.

## Rejected alternatives

The three vocabularies of the first attempt.

## Consequences

The conformance suite and every SDK use these words and no others. They cite
this record.
