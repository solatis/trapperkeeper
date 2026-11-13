# Decisions

Open decisions only. An entry says what needs deciding, the options and when
the choice falls due. Once a decision is made it becomes a record in `adr/`
and leaves this file. Decisions already carried by `principles.md` or
`architecture.md` (local evaluation, ephemeral sensors, schema-agnostic server,
immutable rules) are not repeated.

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

One Rust core serves every SDK (`adr/0038`), and Go is the hard case for it.
`go get` fetches source with no build step, so native code means cgo with a
Rust toolchain on the user's machine, or prebuilt libraries committed per
platform. The third option is a second implementation in Go, held to the core
by the conformance suite. If Go binds to the core, CI can publish the SDK
through a read-only mirror repository that carries the generated artifacts,
so development stays in the monorepo. A pure Go SDK publishes from a monorepo
subdirectory with prefixed tags. Decide when a Go SDK is scheduled.

## What the database stores for a rule

A rule is written as text, and a sensor receives its tree (`adr/0033`). The
database can store the text, the tree or both. Decide with the milestone
"Rules are managed through the API".

## What an event carries

An adapter is asked only for the fields the rules read (`adr/0035`), so an
event can carry the whole record or those fields. A rule that could not be
evaluated on a record is reported upstream with a rate limit in the SDK
(`adr/0005`), and the form of that report and of its limit is open too.
Decide with the milestones "Sensor events are ingested" and "A Rust pipeline
reports matches".

## The one-row path

One row runs the same plan as a batch (`adr/0035`) and pays the interpreter's
fixed cost per step. The form of the one-row path is open. So is whether the
JVM gets an executor of its own for pipelines that hand over one record per
call. Decide with the milestone "The evaluator".

## Host engines: Polars and Spark

On Polars the rules run in our executor until a measurement says otherwise,
and the other option is translation into Polars expressions (`adr/0040`). On
Spark the rules are translated, and how the expressions are built is open.
Spark's internal column route, which reads its column vectors in place, is
open as well. Decide each when its adapter is scheduled.

## Private attributes of a library

An adapter looks fields up in the cheapest way its framework allows
(`adr/0036`). For pandas the public route to the columns measured about 500
times the cost of its private one. Whether an adapter may use a library's
private attributes is open. Decide with the milestone "Python sensor SDK".

## Instruction sets on x86_64

Every measurement behind the evaluator's design comes from one ARM machine
(`adr/0035`). Which instruction sets the loops are compiled for on x86_64 is
open. Decide with the milestone "The evaluator", from measurements on the CI
targets.

## Timestamps in the rule language

The rule language has no timestamp type (`adr/0041`). Rules about time need
one, with comparisons between timestamps and a function for the current time.
Decide when a rule needs them.
