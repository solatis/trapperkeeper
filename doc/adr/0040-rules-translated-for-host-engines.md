# 0040. Rules translated for host engines

Date: 2026-11-13.

## Context

Spark has an expression engine of its own, and it gives a library no public
access to its column memory.

## Decision

A host engine may run a rule in its own executor. This is for data in flight
only. The sensor never runs a job of its own (owner, 2026-11-13).

On Spark the rules are translated into Spark's expressions. Nothing of ours
is called per row (owner, 2026-11-13).

On Spark an event is a count, the first cause and a bounded sample of the
matched rows. It arrives with the user's own Spark action. A `throw` aborts
the job after Spark's task retries. A running batch query never sees a rule
change or a pause, and a streaming query sees one per micro-batch.

A rule literal never reaches a host's parser unescaped.

On Polars the rules run in our executor until a measurement says otherwise.

## Rejected alternatives

Our executor through a user function on Spark. On PySpark it added more than
ten times what translation added, and on the JVM it is a call per row.

An Arrow export of a Spark dataframe, because it collects the data on the
driver.

Both routes with the user's pick, because two routes must then be held to the
same outcome.

## Consequences

On Spark the host's executor defines a rule's outcome, and the fixtures hold
it to ours on a pinned Spark version. How the expressions are built is
decided when a Spark adapter is scheduled (`decisions.md`).
