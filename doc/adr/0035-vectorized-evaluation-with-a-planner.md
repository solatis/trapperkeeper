# 0035. Vectorized evaluation with a planner

Date: 2026-11-26.

## Context

A sensor evaluates rules inside pipelines that process hundreds of millions
of records a day. A rule arrives at run time as data, so no compiler sees it.

## Decision

Evaluation is vectorized, column at a time. "This is premature optimization"
is not an argument in this design (owner, 2026-11-12). A planner is part of
the design from the start (owner, 2026-11-12).

A sensor over named numpy arrays or over a Spark dataframe is as fast as
possible. A sensor over nested JSON, row by row, is slow and works (owner,
2026-11-12, 2026-11-13).

The executor is a vectorized interpreter. It runs one program per rule set
over chunks of up to 2048 rows, with one call per step per chunk (owner,
2026-11-13). No measurement chose 2048, and DuckDB uses the same number. The
measurements on the CI targets also decide the chunk size (`decisions.md`).

The planner runs when the sensor fetches its rules. The planner makes one
program of the whole rule set. The program reads a field once, even when
several rules use it. The program shares the lookup of paths that have a
common prefix. The planner fixes the list of fields that the sensor asks an
adapter for.

Bind works once per schema of a source. Batches whose columns have the same
underlying types run the same compiled loops. Bind picks one compiled loop per
step from the column descriptions (`adr/0036`), and the sensor keeps the result
of bind for each schema of a source (owner, 2026-11-16). An invocation builds no
program.

The loops are compiled when an SDK is built, one per type and layout, from
generic code. Nothing is compiled at run time (owner, 2026-11-13).

For each "no value" convention, one loop turns the source's marking into a
bitmap per chunk. A comparison loop reads only that bitmap and never sees the
convention.

The loops over values that lie one after the other use `std::simd`, and the
engine crate turns on the unstable feature `portable_simd` for it. A strided
column keeps a plain loop (owner, 2026-11-26). Every amd64 build targets
`x86-64-v3` (`adr/0045`).

A single row runs the same plan as a batch.

Reordering rules by measured cost and match rate is a possible later
optimization, with `observe` rules last (owner, 2026-11-12).

## Rejected alternatives

Row-at-a-time evaluation, because it limits performance.

Building the loops first and adding a planner later, because the planner and
the loops are designed together.

A JIT that compiles a rule at run time. On the machine that the Consequences
name, the JIT was about 4 to 8 times faster on one row and 5 times slower on
batches. Cranelift, the code generator of the JIT that was measured, rates
Windows on ARM as its lowest tier.

An executor compiled once per kind of source, because it ran at the same
speed as the interpreter.

A second evaluator for single rows, because two implementations must agree on
every edge case.

## Consequences

From about 256 rows the interpreter ran within about 20 percent of
hand-written Rust. One row pays the interpreter's fixed cost per step.

Every measurement behind the evaluator's design, in this record and in records
0036 and 0039, ran on one Apple M3 Max in a MacBook Pro, and each is meant as a
ratio between two approaches. No number exists for x86_64. It is measured when
code exists (owner, 2026-11-13).

The number of compiled loops grows with the types and the layouts.
