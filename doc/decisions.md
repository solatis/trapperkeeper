# Decisions

Holding pen. Each entry names where it moves once that code exists; then it is
deleted here. Decisions already carried by `principles.md` or `architecture.md`
(local evaluation, ephemeral sensors, schema-agnostic server, immutable rules)
are not repeated.

## Open

### Event store engine

Needs: append-heavy time-ordered writes, retention independent of rule state,
and real querying (filter and aggregate by time, rule, tag, metadata) for the
event viewer and the AI agent. Options: the relational database (one
dependency, weakest at volume); append-only files plus an index (simplest
writes, weak querying); a dedicated time-series or columnar engine (best fit,
one more dependency). Decide when the event query API is designed. Because
events embed rule snapshots, the choice is isolated from the rest of the store.

## Settled, awaiting code

### Rust for the server side

The web server and the APIs are written in Rust. Reasons: performance, which is
extremely important for this project; correctness; a garbage-collected
language such as Go is less suitable for FFI-style use, which keeps the shared
evaluator core option open; Leon's background is heavily C++ and Haskell; and
Rust is clearly gaining ground for this kind of project with LLM providers and
the wider community (Anthropic, OpenAI, Cloudflare, the Linux kernel). SDK
languages are a separate, open decision. Moves to: root `AGENTS.md`, with the
first crate.

### Monorepo

All components are developed in this one repository, if at all possible.
Cross-cutting changes are expected to be common and are far easier to manage
as a single PR, and what is released together belongs in one repository; all
components are released together. Exception: an SDK language whose publishing
requires a standalone repository, decided case by case. Target SDK languages:
Python, Java, Go, Rust, Node.js. Moves to: root `AGENTS.md`, with the first
code.

### Management API is the single write path

The archive let the web UI write to the store directly; a CLI and an agent
would each have needed their own path. One API owns validation, authorization
and audit no matter who acts. Moves to: management API component.

### AI agent is an external client, and manages rules only

Inside the API server it would couple model latency, cost and prompt-injection
exposure to the server and create a call cycle (server launches agent, agent
calls server). As a client with its own principal it gets the same checks as a
human, can be swapped for an external agent, and is optional to deploy.
Agent-authored rules start as `observe`; promotion needs a human. Moves to:
agent component.

### Tenant-aware model, single-tenant operation

Every entity, credential and event carries a tenant id and every query is
tenant-scoped, but we ship one default tenant with no tenant administration,
quotas or isolation guarantees. Retrofitting tenancy would touch
authentication and every entity; carrying the id costs almost nothing. Moves
to: store and management API components.

### Events embed rule snapshots

Rules are versioned and retired while events outlive them as audit records.
Embedding the rule as evaluated makes events self-contained and lets the event
log have its own retention and engine. Cost: larger events, acceptable since
only matches are reported. Moves to: sensor API component.

### One vocabulary for failure behavior

The archive had three. Per sensor, when the server is unreachable:
`pass-through` (default), `halt`, `cached`. Per condition, on missing field
and on coercion failure: `skip` (default), `match`, `fail`. Null is missing; an
uncoercible value is a coercion failure, never missing. Moves to: conformance
suite and SDK components.

### Conformance fixtures are the evaluation spec

The archive specified semantics in prose and the prose contradicted itself.
Fixtures (rule, record, expected outcome) are reviewed once and enforced in
every SDK. Field types: `int`, `float`, `string`, `boolean`, `any`. Moves to:
conformance suite.
