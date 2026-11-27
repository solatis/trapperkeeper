# 0035. Vectorized evaluation with a planner

Date: 2026-11-27.

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

The program is a flat list of steps. Each step writes its result for a chunk
into one slot, and a later step reads it from there. A subexpression that
several rules share is one slot, and a field that several rules read is one
column with one bitmap of the rows that have a value (owner, 2026-11-27). The
program gives its fields as a tree of paths, so an adapter looks up a shared
prefix once. The caller hands one column per field in the order of the
program. A field that the source lacks has no column, and bind makes it a null
on every row (owner, 2026-11-27).

The result of a step for a chunk is three bitmaps: the rows that are known,
the rows that are true and the rows that faulted (owner, 2026-11-27). Both
operands of `and` and `or` run on every row, and the logic step clears the
faults of the rows that the left operand decided. The owner expects this to
be faster than a run of the right operand on only the rows that the left one
left open. The owner expects it also where a guard such as `y != 0` decides
most rows (owner, 2026-11-27).

A run gives for each rule a bitmap of the matched records, a bitmap of the
faulted records, and the faulted records with their faults (owner,
2026-11-27).

The loops are compiled when an SDK is built, one per type and layout, from
generic code. Nothing is compiled at run time (owner, 2026-11-13). A
comparison of two number fields has one loop per pair of number types, which
reads both columns in place. Generic code and a macro produce the pairs, so
no pair is written by hand (owner, 2026-11-27).

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

A tree of operator nodes per rule, because it shares only the reads of fields.

One byte per row with four states as the result of a step, because a logic
step would then run about eight times as many operations as on bitmaps.

Widening each chunk of two number columns into a scratch buffer of a common
type, because the loops read a source in place, with no format between the
source and the loop (`adr/0036`).

Running the right operand of `and` and `or` only on the rows that the left
operand left open, because every loop would then take a selection of rows.

A map from path to column as the input of a run, because it costs a lookup per
field per batch and leaves the sharing of prefixes to the adapter.

A kind of column that is null on every row, for a field that the source lacks,
because bind then cannot tell that the field is absent, and the description
would gain a kind that holds no memory.

One byte per record that holds the outcome and the fault, because it takes
eight times the memory of a bitmap. The bitmaps need a list of the faulted
records beside them for the kind of each fault, and that list stays empty when
no record faults.

## Consequences

From about 256 rows the interpreter ran within about 20 percent of
hand-written Rust. One row pays the interpreter's fixed cost per step.

Every measurement behind the evaluator's design, in this record and in records
0036 and 0039, ran on one Apple M3 Max in a MacBook Pro, and each is meant as a
ratio between two approaches. No number exists for x86_64. It is measured when
code exists (owner, 2026-11-13).

The number of compiled loops grows with the types and the layouts.
