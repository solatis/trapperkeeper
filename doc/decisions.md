# Decisions

This file holds open decisions only. An entry says what needs deciding, the
options and when the choice falls due. Once a decision is made it becomes a
record in `adr/` and leaves this file. Decisions already carried by
`principles.md` or `architecture.md` (local evaluation, ephemeral sensors,
schema-agnostic server) are not repeated.

## Event store engine

Needs: append-heavy time-ordered writes, retention independent of rule state,
and real querying (filter and aggregate by time, rule, tag, metadata) for the
event viewer and the AI agent. Options: the relational database (one
dependency, weakest at volume); append-only files plus an index (simplest
writes, weak querying); a dedicated time-series or columnar engine (best fit,
one more dependency). Decide when the event query API is designed. Because
events embed rule snapshots (`adr/0004`), the choice is isolated from the rest
of the store.

## How Go embeds the core

One Rust core serves every SDK (`adr/0038`), and Go is the hard case for it. `go
get` fetches source with no build step, so native code means cgo with a Rust
toolchain on the user's machine, or prebuilt libraries committed per platform.
CI can publish the SDK through a read-only mirror repository that carries the
built libraries, so development stays in the monorepo. Decide when a Go SDK is
scheduled.

## What the database stores for a rule

A rule is written as text, and a sensor receives its tree (`adr/0033`). The
database can store the text, the tree or both. Decide with the milestone
"Rules are managed through the API".

## How a rule records that the agent wrote it

A rule that the AI agent wrote carries that fact as a property of the rule in
the database. The form of that property is open. Decide with the milestone
"Rules are managed through the API".

## What an event carries

An adapter is asked only for the fields the rules read (`adr/0035`), so an event
can carry the whole record or those fields. The SDK logs a warning for a fault,
sends a report upstream and limits the rate of repeated warnings (`adr/0005`),
and the form of that report and of its limit is open too. Decide with the
milestones "Sensor events are ingested" and "A Rust pipeline reports matches".

## The one-row path

One row runs the same plan as a batch (`adr/0035`) and pays the interpreter's
fixed cost per step. The form of the one-row path is open. Decide with the
milestone "The evaluator".

## Host engines: Polars and Spark

Polars runs the rules in our executor (`adr/0040`). Whether Polars moves to
translation into Polars expressions is open, and a measurement decides it. On
Spark, how the expressions are built is open. Spark's internal column route,
which reads its column vectors in place, is open as well. A native integration
with a host engine, such as an executor for Spark built for performance, stays
possible. Decide each when its adapter is scheduled.

## Private attributes of a library

An adapter looks fields up in the cheapest way its framework allows
(`adr/0036`). For pandas the public route to the columns measured about 500
times the cost of its private one. Whether an adapter may use a library's
private attributes is open. Decide with the milestone "Python sensor SDK".

## Instruction sets on x86_64

Every measurement behind the evaluator's design comes from one ARM machine
(`adr/0035`). Which instruction sets the loops are compiled for on x86_64 is
open, and so is the chunk size of 2048 rows, which no measurement chose.
Decide both with the milestone "The evaluator", from measurements on the CI
targets.

## Patterns on a UTF-32 column

A column of numpy's `U` type is read in place as UTF-32 (`adr/0041`), and the
`regex` crate matches UTF-8 only. The options are to convert a chunk to UTF-8
before the pattern loops run, or to turn the rule off for such a column and
report the column as a fault (`adr/0005`). Decide with the issue that adds the
pattern loops.

## Timestamps in the rule language

The rule language has no timestamp type (`adr/0041`). Rules about time need
one, with comparisons between timestamps and a function for the current time.
Decide when a rule needs them.

## How the agent signs in

The options for the AI agent are a user of its own, a service token, or acting
on behalf of a user. Decide when the agent is scheduled.

## Whether a rule is immutable

An edit makes a new rule with a new id (`adr/0033`). Every event already carries
a snapshot of its rule (`adr/0004`), so what immutability buys is open. The
options are to keep rules immutable or to let an edit change a rule in place.
`adr/0033` and `principles.md`, 6 stay as they are until the choice is made.
Decide with the milestone "Rules are managed through the API".

## The template engine of the web UI

The server renders the web UI through a template engine (`adr/0015`). The
options are askama and minijinja. Decide with the first page, in the milestone
"Operators work in the browser".
