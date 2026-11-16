# 0037. A batch equals its single records

Date: 2026-11-16.

## Context

Pipelines hand over batches, and some hand over one record per call.

## Decision

The batch call is the primary one, and the call for one record wraps it
(owner, 2026-11-12).

A batch gives each record the outcome it gets when it is evaluated alone.
Batching is an optimization (owner, 2026-11-13). A property test of the
executor checks that a batch gives each record the outcome it gets alone.

The default call returns the caller's own object without the records that a
`drop` rule matched. Before it returns, the sensor has recorded the matches of
the `observe` rules as events for reporting. Each SDK shapes the call for its
language (owner, 2026-11-13).

`throw` raises once per call, and the exception carries the whole result
(owner, 2026-11-12, 2026-11-13). To the outside, the evaluation of a batch
behaves as if it stopped at the first row on which a rule throws. The executor
may already have evaluated later rows, because it works on chunks without a
branch per row, and a throw is treated as unlikely. The result defines nothing
for the other rules of the thrown row (owner, 2026-11-16).

## Rejected alternatives

Turning a rule off for a whole batch because of one record, because one
record on which the rule meets a fault, such as a value of another type, would
change the outcome of the other records of its batch.

An exception that names the throwing rules only, because the result exists
anyway and the caller can use it.

Masks as the default result, because a caller wants its own object back.

## Consequences

The conformance runner runs each entry as single records and as one batch
(`adr/0006`). A user who holds single records gets a lower cost per record by
batching them, because one row pays the interpreter's fixed cost per step
(`adr/0035`).
