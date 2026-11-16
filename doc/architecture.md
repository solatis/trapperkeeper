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

TrapperKeeper has two planes, and they behave differently. The **data plane**
(sensor SDK, sensor API) is high-volume, machine-to-machine, and must never hurt
the host pipeline. The **control plane** (management API and its clients) is
low-volume, human-or-agent driven, and is where validation, authorization and
audit live.

## Components

### Sensor SDK

Pipeline code embeds the sensor SDK. The sensor syncs the rules for its scope
tags, evaluates them locally, buffers events and reports them in batches.

- A sensor receives each rule as a validated tree and does not parse rule text
  (`adr/0033`).
- Sampling is applied above the evaluation of a rule (`adr/0033`).
- Buffering is explicit. A flush happens inline or on request. The engine
  works on the calling thread unless the user turns its pool on (`adr/0039`).
- Each sensor chooses one of three behaviors for an unreachable server. Under
  `pass-through`, the default, the rules are off and the records flow. A
  sensor opts into `halt`, and under `halt` the sensor raises into the
  pipeline. Under `cached` the sensor keeps using the last rules it synced.
- Framework adapters sit on top. They build views of the caller's data
  (`adr/0036`) and take metadata from schedulers and orchestrators. On Spark
  the rules are translated (`adr/0040`).

### Sensor API

The sensor API is the endpoint that sensors call. It keeps no state, except
that each instance holds in memory the throttle for its writes to the
seen-scope index. It has two operations.

- **sync rules**: sync returns the rules whose scope tags match the sensor's.
  The request is conditional on a version tag derived from the active rule
  set, and an unchanged set gets an empty response. The sync call also records
  the sensor's scope tags in the seen-scope index (`adr/0034`).
- **report events**: a sensor reports events in batches, and the response
  gives a result per event, so part of a batch can succeed.

Every request carries an API key (`adr/0027`). The sensor API keeps no
sessions, so its instances scale horizontally without coordinating.

### Management API

The management API is the single write path for configuration (`adr/0001`),
which covers rules, API keys, users and roles, and the emergency pause. It is
also the read path for events, which it filters and aggregates by time, rule,
tag and metadata. It owns validation, authorization and the audit trail. It is
also exposed as a tool surface for agents. Its code is in the server, which
`server/AGENTS.md` describes.

### Rule engine core

The engine is the library with which the server parses and validates a rule
when the rule is created, and which pipelines embed to evaluate rules. See
`engine/AGENTS.md`.

### Web UI

The web UI is the management API's client for people. It offers the rule
editor, the event viewer, user and key management, the pause switch and the
conversation surface for the AI agent. The server renders it, and there is no
single-page application (`adr/0015`).

### AI agent

The AI agent is a separate component and a client of the management API
(`adr/0002`, which also states what the agent manages). A client invokes it,
such as the web UI, the CLI or an alert hook, and the API never invokes it.
The agent queries events to troubleshoot. It creates, scopes, samples and
pauses rules.

The agent may create rules of any kind. How a rule records that the agent wrote
it is open (see `decisions.md`). Rules are immutable, so an edit is a new rule
with a new id, and an operator undoes a rule change of the agent by pausing or
deleting the rule (`principles.md`, 6). The contents of a record are untrusted
input to the model.

### Store

- Relational state: the configuration, the seen-scope index (`adr/0034`) and
  the audit trail. It lives in the server's store module, which
  `server/src/store/AGENTS.md` describes.
- Event log: append-only, time-ordered, with retention independent of rule
  state. An event embeds the rules it matched (`adr/0004`). The engine of the
  event log is open (see `decisions.md`).

### Conformance suite

The conformance suite is a set of language-neutral fixtures. Each entry holds a
rule, records and the expected outcome of each record. The fixtures define what
each kind of expression gives and when a rule cannot be evaluated (`adr/0006`).
Every SDK must pass them, and they are the specification of evaluation.

## Flows

**Rule change.** A principal submits a rule to the management API -> the
server parses and validates it -> the server stores the rule, which never
changes afterwards, and an edit is stored as a new rule with a new id -> the
version tag of the active set changes -> each sensor picks the rule up at its
next sync (seconds, eventually consistent).

**Record evaluation.** The pipeline hands a batch to the sensor, and a call for
one record wraps the batch call (`adr/0037`) -> sample -> the adapter presents
the fields the rules read -> evaluate -> apply the actions (`observe` records an
event, `drop` removes the records that a `drop` rule matched from the object
that the call returns, and `throw` raises once per call with the whole result in
the exception) -> buffer events -> batch report -> the server stamps the receive
time and appends to the event log.

**Troubleshooting.** Operator asks agent -> agent queries events -> agent
creates a narrow `observe` rule -> sensors pick it up -> agent reads resulting
events -> agent reports findings.

**Server unreachable.** Sensor applies its configured failure behavior, logs,
retries sync at a fixed interval. Unreported events beyond the buffer are lost.

## Cross-cutting

- **Identity.** A sensor authenticates with an opaque API key, which the server
  stores as a hash (`adr/0027`). A person logs in for signed tokens, and a
  per-user revocation cutoff revokes them (`adr/0025`). Every entity and every
  credential carries a tenant id.
- **Identifiers and time.** Every entity has a time-ordered unique id
  (`adr/0020`). Storage and the wire carry UTC, and only the presentation
  shows local time. An event keeps both the sensor's timestamp and the
  server's.
- **Validation.** The server validates its own schemas strictly: rules, settings
  and API requests. A rule treats the user's data leniently. A rule treats a
  missing value as a null, and a rule that meets a fault on a record does not
  apply to that record (`adr/0005`).
- **Operations.** One distributable carries the subcommands `serve`, `migrate`
  and `reset-superadmin`. A secret setting may come from the settings file or
  the environment, and on the command line the server accepts it with a warning
  (`adr/0013`). Today a proxy terminates TLS. The server terminates TLS itself
  in a later milestone, and it can then generate self-signed certificates if the
  user wants them, for example to secure the link between a proxy and the
  server.
- **Observability.** Structured logs, metrics endpoint, liveness and readiness
  endpoints on every service.
