# Principles

This file lists the trade-offs we resolve the same way every time. A principle
that gives something up says what under "Cost".

## 1. Least intrusive by default

An observability tool must be invisible when it works and harmless when it
breaks. Every default degrades toward pass-through. A rule treats a value that
is not there as a null, which turns no rule off. A rule that meets a fault on a
record does not apply to that record, and the SDK reports the fault
(`adr/0005`). When the server is unreachable, the rules are off and records
flow. Strictness is opt-in per sensor.

Cost: events can be lost. A crash of the native core is an exception to
"harmless when it breaks". The loops of the core read the memory of a source in
place, so the core can crash the process (`adr/0038`).

## 2. Schema-agnostic server

The server never learns the shape of user data. Rules reference field paths
that are resolved against the actual record at evaluation time. Schema drift
needs no coordination.

Cost: a typo in a field path matches nothing, and nothing reports it
(`adr/0005`). Mitigation belongs in tooling (event-informed suggestions), not
in a registry.

## 3. Ephemeral sensors

Sensors live as long as the job. A sensor has no registration, no identity, no
heartbeat and no server-side inventory, and it keeps no state that outlives the
process.

Cost: no cross-run continuity, no "which sensors exist" view other than what
event metadata reveals.

## 4. Evaluate locally, manage centrally

Evaluation happens in the pipeline process. A record therefore costs no network
round-trip, the server is no bottleneck, and a sensor set to `cached` keeps
evaluating while it is disconnected. Authoring, validation, authorization and
audit happen in the management API.

Cost: every SDK carries the same Rust core (`adr/0038`), and a host engine that
runs translated rules must agree with it (`adr/0040`). The conformance suite
exists to pay this cost.

## 5. Deterministic semantics

The same rule on the same record gives the same outcome in every SDK. A batch
gives each record the outcome it gets alone (`adr/0037`). A source says what "no
value" is, so two frameworks can differ on the same numbers (`adr/0036`).
Optimizations (batching, vectorization, a later reordering of rules) may change
cost, never results.

## 6. Reversible operations

Rules are immutable versions. `observe` lets an operator test a rule in
production. A global pause turns every rule off. An operator undoes a change to
a rule, made by a human or by the agent, by pausing or deleting the rule.
Because every such change can be undone, the AI agent may change rules on its
own.

## 7. Simplicity

Build the simple thing, record what was deferred as a decision, revisit on
evidence. We work incrementally and make no large upfront plans, and a design
question is settled when the code that needs it is written. The plan for one
issue is written right before its code and deleted once the code lands, and such
a plan does not count as an upfront plan. Each component gets the kind of test
that `adr/0018` assigns it: unit, integration or end-to-end. We prefer property,
behavioral and data-driven tests. Each concern has one convention.

## 8. Backwards compatibility is not a concern

Nothing is kept compatible (owner, 2026-10-31). The rule format, the wire
formats of every API, the database schema and its migrations, configuration
keys and the SDK interfaces change in place: no deprecation periods, no version
negotiation, no migration path for existing data or deployments. The old shape
is replaced, not kept beside the new one.

Cost: any deployment or data may break on upgrade and is rebuilt rather than
migrated.
