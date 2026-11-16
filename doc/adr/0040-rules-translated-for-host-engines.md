# 0040. Rules translated for host engines

Date: 2026-11-16.

## Context

Spark has an expression engine of its own. Spark gives a library no public
access to its column memory.

## Decision

A host engine may run a rule in its own executor, and only on data in flight,
that is, on the records as the user's own job processes them. The sensor never
starts a job of its own, such as a map/reduce job on Spark for analysis (owner,
2026-11-13).

On Spark the Spark adapter translates the rules into Spark expressions. No
TrapperKeeper code runs per row (owner, 2026-11-13).

Whichever executor runs a rule, the rule behaves the same to the outside. Spark
is an example, and the first evaluator for a host engine is written for the
engine that users ask for (owner, 2026-11-16). On Spark an event arrives with
the user's own Spark action. A `throw` aborts the job after Spark's task
retries. A running batch query sees no rule change and no pause. A streaming
query sees a rule change or a pause once per micro-batch.

A rule literal never reaches a host's parser unescaped.

On Polars the rules run in our executor until a measurement says otherwise.

## Rejected alternatives

Our executor through a user function on Spark. On PySpark a user function
added more than ten times the cost that translation added. On the JVM a user
function is a call per row.

An Arrow export of a Spark dataframe, because it collects the data on the
driver.

Both routes with the user's pick, because two routes must then be held to the
same outcome.

## Consequences

On Spark the Spark executor defines a rule's outcome. The conformance fixtures
hold that outcome to the outcome of our executor, on a pinned Spark version. How
the expressions are built is decided when a Spark adapter is scheduled
(`decisions.md`).
