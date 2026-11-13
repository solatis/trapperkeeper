# Principles

Trade-offs we resolve the same way every time. Each lists what we give up.

## 1. Least intrusive by default

An observability tool must be invisible when it works and harmless when it
breaks. Every default degrades toward pass-through. A rule that cannot be
evaluated on a record does not apply to that record and is reported
(`adr/0005`). When the server is unreachable, the rules are off and records
flow. Strictness is opt-in per sensor.

Cost: events can be lost.

## 2. Schema-agnostic server

The server never learns the shape of user data. Rules reference field paths
that are resolved against the actual record at evaluation time. Schema drift
needs no coordination.

Cost: a typo in a field path is found only when a sensor reports that the
rule could not be evaluated (`adr/0005`).
Mitigation belongs in tooling (event-informed suggestions), not in a registry.

## 3. Ephemeral sensors

Sensors live as long as the job. No registration, no identity, no heartbeat,
no server-side inventory, no state that outlives the process.

Cost: no cross-run continuity, no "which sensors exist" view other than what
event metadata reveals.

## 4. Evaluate locally, manage centrally

Evaluation happens in the pipeline process: no network round-trip per record,
no server bottleneck, works while disconnected. Everything else (authoring,
validation, authorization, audit) happens in exactly one place.

Cost: every SDK carries the one core (`adr/0038`), and a host engine that
runs translated rules must agree with it (`adr/0040`). The conformance suite
exists to pay this cost.

## 5. Deterministic semantics

Same rule plus same record gives the same outcome in every SDK. Where ordering
matters, tie-breaking is specified. A batch gives each record the outcome it
gets alone (`adr/0037`). A source says what "no value" is, so two frameworks
can differ on the same numbers (`adr/0036`). Optimizations (batching,
vectorization, a later reordering of rules) may change cost, never results.

## 6. Reversible operations

Rules are immutable versions; `observe` exists to test in production; a global
pause turns everything off. Any rule action, by human or agent, can be undone
quickly. This is what makes autonomous agent operation acceptable.

## 7. Simplicity

Build the simple thing, record what was deferred as a decision, revisit on
evidence. Work incrementally: no large upfront plans; a design question is
settled when the code that needs it is written. A plan for one issue,
written right before its code and deleted once it lands, is how that
settling happens; it is not an upfront plan. Each component gets the kind
of test `adr/0018` assigns it, unit, integration or end-to-end, with
properties preferred over hand-picked cases. One convention per concern.

## 8. Backwards compatibility is not a concern

Nothing is kept compatible (owner, 2026-10-31). The rule format, the wire
formats of every API, the database schema and its migrations, configuration
keys and the SDK interfaces change in place: no deprecation periods, no version
negotiation, no migration path for existing data or deployments. The old shape
is replaced, not kept beside the new one.

Cost: any deployment or data may break on upgrade and is rebuilt rather than
migrated.
