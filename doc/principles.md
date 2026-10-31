# Principles

Trade-offs we resolve the same way every time. Each lists what we give up.

## 1. Least intrusive by default

An observability tool must be invisible when it works and harmless when it
breaks. Every default degrades toward pass-through: missing field -> condition
does not match; coercion failure -> condition does not match; server
unreachable -> rules off, records flow. Strictness is opt-in, per condition or
per sensor.

Cost: misconfigurations can fail silently; events can be lost.

## 2. Schema-agnostic server

The server never learns the shape of user data. Rules reference field paths
that are resolved against the actual record at evaluation time. Schema drift
needs no coordination.

Cost: a typo in a field path is only visible as a rule that never matches.
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

Cost: every SDK carries an evaluator, and they must all agree. The conformance
suite exists to pay this cost.

## 5. Deterministic semantics

Same rule plus same record gives the same outcome in every SDK. Where ordering
matters, tie-breaking is specified. Optimizations (short-circuiting, condition
reordering, vectorization) may change cost, never results.

## 6. Reversible operations

Rules are immutable versions; `observe` exists to test in production; a global
pause turns everything off. Any rule action, by human or agent, can be undone
quickly. This is what makes autonomous agent operation acceptable.

## 7. Simplicity

Build the simple thing, record what was deferred as a decision, revisit on
evidence. Work incrementally: no large upfront plans; a design question is
settled when the code that needs it is written. A plan for one issue, written right before its code and deleted once it lands, is how that settling happens; it is not an upfront plan. Prefer integration-level tests and property-based tests over large
unit suites. One convention per concern.
