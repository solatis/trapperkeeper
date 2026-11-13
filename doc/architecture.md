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

- A sensor receives each rule as a validated tree and holds no parser
  (`adr/0033`).
- Sampling is applied above the evaluation of a rule (`adr/0033`).
- Buffering is explicit. A flush happens inline or on request. The engine
  works on the calling thread unless the user turns its pool on (`adr/0039`).
- Behavior when the server is unreachable is configurable per sensor:
  `pass-through` (default: rules off, records flow), `halt` (raise into the
  pipeline), `cached` (keep using the last synced rules).
- Framework adapters sit on top. They build views of the caller's data
  (`adr/0036`) and take metadata from schedulers and orchestrators. On Spark
  the rules are translated (`adr/0040`).

### Sensor API

Stateless endpoint for sensors, except for the write throttle of the
seen-scope index, which each instance keeps in memory. Two operations:

- **sync rules**: conditional on a version tag derived from the active rule
  set; an unchanged set costs an empty response. Filtered by scope tags. The
  sync call also records the sensor's scope tags in the seen-scope index
  (`adr/0034`).
- **report events**: batched, with per-event results so a batch can partially
  succeed.

Every request carries an API key (`adr/0027`); there are no sessions, so
instances scale horizontally with no coordination.

### Management API

The single write path for configuration (`adr/0001`): rules, API keys, users
and roles, emergency pause. Also the read path for events (filter and
aggregate by time, rule, tag, metadata). Owns validation, authorization and the
audit trail. Authorization is a minimum role per route on a ladder
(`adr/0029`). Web UI, CLI and AI agent are equal clients; none of them touches
the store. Also exposed as an agent tool surface. Its code is in the server;
see `server/AGENTS.md`.

### Rule engine core

The engine crate holds the rule model, its JSON form and its validation, and
later the evaluator. Rules are validated strictly at creation so that
evaluation can assume valid rules. See `engine/AGENTS.md`.

### Web UI

Human client of the management API: rule editor, event viewer, user and key
management, pause switch, and the conversation surface for the AI agent.
Rendered on the server, with no single-page application (`adr/0015`).

### AI agent

Separate component, client of the management API with its own principal and
role. Invoked by a client (web UI, CLI, alert hook), never by the API itself.
Queries events to troubleshoot; creates, scopes, samples and pauses rules. It
manages rules, not sensors: new instrumentation points in pipeline code are at
most a suggested patch for a human.

Guardrails come from the rule model: agent-authored rules start as `observe`
and are tagged as agent-authored; promotion to `drop` or `throw` needs a human;
immutable versions and pause make every agent action reversible. Record
contents are untrusted input to the model.

### Store

- Relational state: rules, keys, users, tenants and the audit trail. It lives
  in the server's store module, on SQLite, Postgres or MariaDB, and entities
  are deleted softly (`adr/0024`). See `server/src/store/AGENTS.md`.
- Event log: append-only, time-ordered, independent retention. Events embed
  rule snapshots so they are self-contained. Engine choice is open (see `decisions.md`).

### Conformance suite

Language-neutral fixtures: a rule, records and the expected outcome of each.
The fixtures define what each kind of expression gives and when a rule cannot
be evaluated (`adr/0006`). Every SDK must pass them, and they are the
specification of evaluation.

## Flows

**Rule change.** Principal submits rule to management API -> parsed and
validated -> stored as new immutable version -> active-set version tag changes
-> each sensor picks it up on next sync (seconds, eventually consistent).

**Record evaluation.** Pipeline hands record to sensor -> sample -> the
adapter presents the fields the rules read -> evaluate -> apply action
(`observe`: nothing; `drop`: tell the caller to discard; `throw`: raise) ->
buffer event -> batch report -> server stamps receive time and appends to
event log.

**Troubleshooting.** Operator asks agent -> agent queries events -> agent
creates a narrow `observe` rule -> sensors pick it up -> agent reads resulting
events -> agent reports findings, optionally proposes promotion.

**Server unreachable.** Sensor applies its configured failure behavior, logs,
retries sync at a fixed interval. Unreported events beyond the buffer are lost.

## Cross-cutting

- **Identity.** Sensors: opaque API keys stored hashed (`adr/0027`).
  People: signed tokens from login, revoked per user by a cutoff
  (`adr/0025`). Every entity and credential carries a tenant id. The
  superadmin is a user of the system tenant `$tk` (`adr/0028`).
- **Identifiers and time.** Time-ordered unique ids for all entities. UTC in
  storage and on the wire; local time only at presentation. Both sensor and
  server timestamps are kept.
- **Validation.** Strict for our own schemas (rules, config, API requests);
  best-effort for user data: a rule that cannot be evaluated on a record does
  not apply to it (`adr/0005`).
- **Operations.** One distributable with subcommands per role; the first
  serve on an empty database migrates it, and services refuse to start on
  pending migrations, which an explicit migration step applies; config
  precedence CLI over environment over file; secrets may come from the file
  or the environment, the command line warns (`adr/0013`); TLS terminated
  directly or by a proxy.
- **Observability.** Structured logs, metrics endpoint, liveness and readiness
  endpoints on every service.
