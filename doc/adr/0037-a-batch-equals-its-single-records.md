# 0037. A batch equals its single records

Date: 2026-11-13.

## Context

Pipelines hand over batches, and some hand over one record per call.

## Decision

The batch call is the primary one, and the call for one record wraps it
(owner, 2026-11-12).

A batch gives each record the outcome it gets when it is evaluated alone.
Batching is an optimization (owner, 2026-11-13). A property test holds this
once the executor exists.

The default call returns the caller's own object with the dropped items
removed and the observations done. Each SDK shapes the call for its language
(owner, 2026-11-13).

`throw` raises once per call, and the exception carries the whole result. The
first row that throws is the only row that is thrown, and for that row the
rules that do not throw are not important (owner, 2026-11-12, 2026-11-13).

## Rejected alternatives

Turning a rule off for a whole batch because of one record, because one odd
record would change its neighbours.

An exception that names the throwing rules only, because the result exists
anyway and the caller can use it.

Masks as the default result, because a caller wants its own object back.

## Consequences

The fixtures run each entry in both forms (`adr/0006`). A user who holds
single records is encouraged to batch them.
