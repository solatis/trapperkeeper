# 0034. Scope tags and the seen-scope index

Date: 2026-11-11.

## Context

A rule carries scope tags and a sensor declares its own, so a rule reaches
only the sensors it is meant for. Sensors have no registration and no
inventory (`principles.md`, 3), so nothing tells an operator which scopes
exist, and a typo in a rule's scope is a rule that reaches no sensor. The
observe-first test flow cannot reveal it, because such a rule produces no
event at all. The owner asked for the process to be settled before the
engine fixes the form of the tags (owner, 2026-11-11).

## Decision

Scope tags are a map from key to value, on a rule and on a sensor. They are
free-form: nothing registers them (owner, 2026-11-11). A tag is a dimension
of a deployment, such as the environment, the pipeline or the team. Per-run
context, such as a run id or a host, is event metadata and never a tag.

A rule reaches a sensor when every tag of the rule is present on the sensor
with the same value. A rule with no tags reaches every sensor.

A rule and a sensor carry at most 8 tags, and a key and a value are 1 to 64
characters (owner, 2026-11-11: caps in the engine). The engine validates a
rule's tags and the sync endpoint a sensor's.

The server keeps a seen-scope index: per tenant, every tag map a sensor has
synced with, and when it was last seen (owner, 2026-11-11). The index is a
table of the relational store. A row is the tenant id, the tuple and the
last-seen time, and nothing else: the tenant id is the one the sync's API
key resolved to (`adr/0003`, `adr/0027`), and the row holds neither the key
nor a key id. The row is keyed by tenant and tuple, not by key and not by
sensor, because a sensor has no identity (`principles.md`, 3). Sensors that
share one API key and declare different scopes produce one row each, and
sensors with different keys and the same scope share one row (owner,
2026-11-11: sharing a key with different scopes must be accounted for). The
sync endpoint writes a tuple the first time it sees it and refreshes its
last-seen time at most once an hour per server instance, through an
in-memory map of what that instance wrote. A tuple is the tags in canonical
form, keys sorted, as JSON.

A tuple counts as live when seen within the live window and is deleted once
older than the retention. Both are settings of the server,
`scopes.live_window` and `scopes.retention`, with defaults of 7 and 30 days
(owner, 2026-11-11: configurable). Housekeeping is a periodic delete in
`serve` of the tuples older than the retention; the live test is a filter on
the last-seen time and needs no housekeeping. Per-tenant values come later
if a tenant needs them.

The UI offers the seen keys and values when a rule is scoped and shows each
scope's last-seen time. The management API refuses a rule whose scope
matches no live tuple, for every caller, with no override (owner,
2026-11-11). In every response that returns rules, it flags a rule whose
scope matches no live tuple any more, by joining the rules against the
index (owner, 2026-11-11).

## Rejected alternatives

A registry of tags managed through the API, because it is registration by
another name, an admin chore, and a stale registry lies.

Deriving the scopes from events, because it lags and shows only the sensors
that matched something.

An in-memory index alone, because the window exists for sensors that sync
rarely, which a restart would erase, and because the sensor API runs as
several instances that would each know different scopes.

A write on every sync, because it amplifies writes for a precision the
window does not need.

An override of the refusal for people, because strictness can be relaxed
later and the reverse cannot (owner, 2026-11-11).

Per-tenant windows now, because they need a tenant settings table, an API
and a UI, where two server settings need none.

A `sensor_id` in the row, because sensors have no identity. An `api_key_id`
in the row, because no caller asks which key announced a scope and rows age
out on their own; it can be added when a use appears.

An announce call beside sync, because sync already carries the tags.

## Consequences

The sensor API writes to the relational store (`adr/0001`). A rule is
created only after a sensor with its scope has synced, so a pipeline is
deployed before its rules are written, and a job that runs less often than
the live window cannot keep rules. A rule whose scope went stale stays in
force and is flagged, not retired. Tag cardinality is the operator's
discipline, bounded per rule and per sensor by the caps and not per tenant.
The engine's ordered serialization of the scope map is the canonical tuple.
The sync wire (M6) carries the sensor's tags. The engine, the server and the
web components cite this record.
