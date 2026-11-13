# 0039. An optional thread pool

Date: 2026-11-13.

## Context

Spark, Flink and Polars already spread data over their own threads. A user
with one process and a large batch has no such host.

## Decision

By default the engine works on the calling thread. A user can turn a pool on
(owner, 2026-11-13).

The planner chooses the split. It splits by groups of rules that share no
field, or by ranges of rows. The groups are computed when the rules are
retrieved, and the choice is made per invocation from the input size (owner,
2026-11-13).

## Rejected alternatives

A pool that is on by default, because it multiplies threads inside a host
that already runs in parallel.

A split by rule only. On 64 rules over a million rows it ran 2.3 to 3.2 times
faster when rules share fields, against 6.3 to 7.5 times for a split by row
range.

## Consequences

A fan-out pays from about 8,000 rows on the machine that measured it. Threads
that read the same read-only data did not slow each other.
