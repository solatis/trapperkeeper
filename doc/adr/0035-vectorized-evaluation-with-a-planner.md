# 0035. Vectorized evaluation with a planner

Date: 2026-11-13.

## Context

A sensor evaluates rules inside pipelines that process hundreds of millions
of records a day. A rule arrives at run time as data, so no compiler sees it.

## Decision

Evaluation is vectorized, column at a time. "This is premature optimization"
is not an argument in this design (owner, 2026-11-12). A planner is part of
the design from the start (owner, 2026-11-12).

Fast things must be fast, and slow things must be possible. A sensor over
named numpy arrays or over a Spark dataframe is as fast as possible. A sensor
over nested JSON, row by row, is slow and works (owner, 2026-11-12,
2026-11-13).

The executor is a vectorized interpreter. It runs one program per rule set
over chunks of up to 2048 rows, with one call per step per chunk (owner,
2026-11-13).

The planner works when the rules are retrieved. It makes one program of the
whole rule set, reads a field that several rules use once, shares the lookup
of paths with a common prefix, and fixes the list of fields to ask an adapter
for.

Bind works once per kind of batch. It picks one compiled loop per step from
the column descriptions (`adr/0036`), and the result is cached. An invocation
does no planning.

The loops are compiled when an SDK is built, one per type and layout, from
generic code. Nothing is compiled at run time (owner, 2026-11-13).

One loop per "no value" convention writes a bitmap per chunk, and a
comparison loop never sees the convention.

One row runs the same plan.

Reordering rules by measured cost and match rate is a possible later
optimization, with `observe` rules last (owner, 2026-11-12).

## Rejected alternatives

Row-at-a-time evaluation, because it limits performance.

Loops without a planner first, because the planner is one design with them.

A JIT that compiles a rule at run time. On one machine it was about 4 to 8
times faster on one row and 5 times slower on batches, and its compiler rates
Windows on ARM as its lowest tier.

An executor compiled once per kind of source, because it ran at the same
speed as the interpreter.

A second evaluator for single rows, because two implementations must agree on
every edge.

## Consequences

From about 256 rows the interpreter ran within about 20 percent of
hand-written Rust. One row pays the interpreter's fixed cost per step.

Every comparison in the records was measured on one Apple M3 Max in a MacBook
Pro and is meant as a ratio between two approaches. No number exists for
x86_64. It is measured when code exists (owner, 2026-11-13).

The number of compiled loops grows with the types and the layouts.
