# 0039. An optional thread pool

Date: 2026-11-13.

## Context

Spark, Flink and Polars already spread data over their own threads. A user
with one process and a large batch has no such host.

## Decision

By default the engine works on the calling thread. A user can turn a pool on
(owner, 2026-11-13).

The planner chooses the split. It splits by groups of rules that share no
field, or by ranges of rows. The planner computes the groups of rules when
the sensor retrieves the rules. The planner chooses between the two splits per
invocation, from the input size (owner, 2026-11-13).

## Rejected alternatives

A pool that is on by default, because it multiplies threads inside a host
that already runs in parallel.

A split by rule only. On 64 rules over a million rows, a split by rule on a
pool of 8 or 12 threads ran 2.3 to 3.2 times faster than a sequential run on
the calling thread, when rules share fields. A split by row range on the same
pool ran 6.3 to 7.5 times faster than the sequential run.

## Consequences

Spreading one invocation over the pool pays off from about 8,000 rows, on the
machine where it was measured. Threads that read the same read-only data did not
slow each other.
