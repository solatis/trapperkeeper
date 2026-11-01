# Architecture

```
 operators --> [web UI] -------+
 CLI / automation -------------+--> [management API] --+
 [AI agent] -------------------+                       |
                                                       +--> [rule engine core]
                                                       +--> [store]
 pipeline code --> [sensor SDK] --> [sensor API] ------+
                    evaluates rules locally
```

Two planes with different characters. The **data plane** (sensor SDK, sensor
API) is high-volume, machine-to-machine, and must never hurt the host pipeline.
The **control plane** (management API and its clients) is low-volume,
human-or-agent driven, and is where validation, authorization and audit live.

## Components

### Sensor SDK

Embedded in pipeline code. Syncs rules for its scope tags, evaluates them
locally, buffers events, reports them in batches.

- Rules arrive pre-compiled and pre-validated; the SDK contains an evaluator,
  not a parser.
- Conditions are evaluated cheapest-first using a deterministic cost model;
  sampling is applied before evaluation.
- Buffering is explicit: no background threads, flush happens inline or on
  request.
- Behavior when the server is unreachable is configurable per sensor:
  `pass-through` (default: rules off, records flow), `halt` (raise into the
  pipeline), `cached` (keep using the last synced rules).
- Framework adapters sit on top: vectorized evaluation for dataframes,
  automatic metadata from schedulers and orchestrators.

### Sensor API

Stateless endpoint for sensors. Two operations:

- **sync rules**: conditional on a version tag derived from the active rule
  set; an unchanged set costs an empty response. Filtered by scope tags.
- **report events**: batched, with per-event results so a batch can partially
  succeed.

Every request is signed with an API key; there are no sessions, so instances
scale horizontally with no coordination.

### Management API

The single write path for everything that is not an event: rules, API keys,
users and roles, emergency pause. Also the read path for events (filter and
aggregate by time, rule, tag, metadata). Owns validation, authorization and the
audit trail. Web UI, CLI and AI agent are equal clients; none of them touches
the store. Also exposed as an agent tool surface.

### Rule engine core

Shared logic behind both APIs: rule validation, compilation to the distributed
form, cost calculation, limits (path depth, nested wildcards, set sizes). Rules
are validated strictly at creation so that evaluation can assume valid rules.

### Web UI

Human client of the management API: rule builder, event viewer, user and key
management, pause switch, and the conversation surface for the AI agent.
Rendered on the server, with no single-page application (`adr/0015`).

### AI agent

Separate component, client of the management API with its own principal and
role. Invoked by a client (web UI, CLI, alert hook), never by the API itself.
Queries events to troubleshoot; creates, scopes, samples and pauses rules. It
manages rules, not sensors: new instrumentation points in pipeline code are at
most a suggested patch for a human.

Guardrails come from the rule model: agent-authored rules start as `observe`
and are tagged as agent-authored; promotion to `drop` or `fail` needs a human;
immutable versions and pause make every agent action reversible. Record
contents are untrusted input to the model.

### Store

- Relational state: rules, keys, users, tenants, audit trail. An embedded,
  zero-config option for evaluation and a server database for production.
- Event log: append-only, time-ordered, independent retention. Events embed
  rule snapshots so they are self-contained. Engine choice is open (see `decisions.md`).

### Conformance suite

Language-neutral fixtures: rule, record, expected outcome. Defines field path
resolution, type coercion, missing-vs-coercion-failure handling and wildcard
semantics. Every SDK must pass it; it is the specification of evaluation.

## Flows

**Rule change.** Principal submits rule to management API -> validated and
compiled -> stored as new immutable version -> active-set version tag changes
-> each sensor picks it up on next sync (seconds, eventually consistent).

**Record evaluation.** Pipeline hands record to sensor -> sample -> resolve
paths, coerce, evaluate -> apply action (`observe`: nothing; `drop`: tell the
caller to discard; `fail`: raise) -> buffer event -> batch report -> server
stamps receive time and appends to event log.

**Troubleshooting.** Operator asks agent -> agent queries events -> agent
creates a narrow `observe` rule -> sensors pick it up -> agent reads resulting
events -> agent reports findings, optionally proposes promotion.

**Server unreachable.** Sensor applies its configured failure behavior, logs,
retries sync at a fixed interval. Unreported events beyond the buffer are lost.

## Cross-cutting

- **Identity.** Sensors: API keys, per-request signatures. Principals:
  authenticated identities with roles. Every entity and credential carries a
  tenant id.
- **Identifiers and time.** Time-ordered unique ids for all entities. UTC in
  storage and on the wire; local time only at presentation. Both sensor and
  server timestamps are kept.
- **Validation.** Strict for our own schemas (rules, config, API requests);
  best-effort for user data, governed by per-condition policies.
- **Operations.** One distributable with subcommands per role; explicit
  migration step, services refuse to start on pending migrations; config
  precedence CLI over environment over file; secrets may come from the file
  or the environment, the command line warns (`adr/0013`); TLS terminated
  directly or by a proxy.
- **Observability.** Structured logs, metrics endpoint, liveness and readiness
  endpoints on every service.
