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

## Sensor SDK evaluator: shared core or per-language

Decided case by case per SDK. Option one: a shared Rust evaluator core with
thin bindings per SDK language. It simplifies a lot (one evaluator, agreement
by construction) but is only viable if it is at least as fast as a native
implementation in that language and does not complicate installation, which is
a big if. Option two: reimplement the rule engine in each SDK language, paid
for by the conformance suite. Weigh the cost of the bindings against the cost
of each reimplementation. Go is the hard case for a shared core: `go get`
fetches source with no build step, so native code means requiring a Rust
toolchain or committing prebuilt libraries per platform. If Go binds to the
core anyway, CI can publish it through a read-only mirror repository that
carries the generated artifacts, keeping development in the monorepo. A pure
Go SDK publishes from a monorepo subdirectory with prefixed tags. Each SDK's
choice becomes a record in `adr/`.
