# 0034. Scope tags and the seen-scope index

Date: 2026-11-11.

## Context

A rule carries scope tags and a sensor declares its own, so a rule reaches only
the sensors it is meant for. Sensors have no registration and no inventory
(`principles.md`, 3), so nothing tells an operator which scopes exist, and a
rule with a typo in its scope reaches no sensor. An operator or the agent first
creates a rule under `observe` and reads its events before giving the rule
another action. A rule with a typo in its scope produces no event, so this flow
cannot reveal the typo. The owner asked for the process to be settled before the
engine fixes the form of the tags (owner, 2026-11-11).

## Decision

Scope tags are a map from key to value, on a rule and on a sensor. Scope tags
are free-form, and nothing registers them (owner, 2026-11-11). A tag is a
dimension of a deployment, such as the environment, the pipeline or the team.
Per-run context, such as a run id or a host, is event metadata and never a tag.

A rule reaches a sensor when every tag of the rule is present on the sensor
with the same value. A rule with no tags reaches every sensor.

A rule and a sensor carry at most 8 tags, and a key and a value are 1 to 64
characters (owner, 2026-11-11: caps in the engine). The engine validates the
tags of a rule, and the sync endpoint validates the tags of a sensor.

The server keeps a seen-scope index. Per tenant, the index records every tag map
that a sensor has synced with, and the last time a sensor synced with that tag
map (owner, 2026-11-11). A tuple is the tags of such a map in canonical form,
keys sorted, as JSON. The index is a table of the relational store. A row holds
the tenant id, the tuple and the last-seen time. The tenant id is the tenant
that the API key of the sync resolved to (`adr/0003`, `adr/0027`). The row holds
neither the key nor a key id. The row is keyed by tenant and tuple, not by key
and not by sensor. Sensors that share one API key and declare different scopes
produce one row each, and sensors with different keys and the same scope share
one row (owner,
2026-11-11: sharing a key with different scopes must be accounted for). The
sync endpoint writes a tuple the first time it sees it and refreshes its
last-seen time at most once an hour per server instance, through an
in-memory map of what that instance wrote.

A tuple is live while a sensor has synced with it within the live window. The
server deletes a tuple once its last-seen time is older than the retention. Both
are settings of the server, `scopes.live_window` and `scopes.retention`, with
defaults of 7 and 30 days (owner, 2026-11-11: configurable). `serve`
periodically deletes the tuples older than the retention. The check whether a
tuple is live filters on the last-seen time and needs no housekeeping.
Per-tenant values come later if a tenant needs them.

The UI offers the seen keys and values when a rule is scoped and shows each
scope's last-seen time. The management API refuses a rule whose scope
matches no live tuple, for every caller, with no override (owner,
2026-11-11). In every response that returns rules, it flags a rule whose
scope matches no live tuple any more, by joining the rules against the
index (owner, 2026-11-11).

## Rejected alternatives

A registry of tags managed through the API, because it is registration under
another name (`principles.md`, 3) and an admin chore. A registry kept by hand
keeps scopes that no sensor declares and misses scopes that sensors declare. A
tag would also have to be created before a pipeline could use it, and the
seen-scope index shows any user which tags the sensors send (owner, 2026-11-16).

Deriving the scopes from events, because it lags and shows only the sensors
that matched something.

An in-memory index alone, because the window exists for sensors that sync
rarely, which a restart would erase, and because the sensor API runs as
several instances that would each know different scopes.

A write on every sync, because it amplifies writes for a precision the
window does not need.

An override of the refusal for people, because an override can still be
added later, and an override that callers already use cannot be taken away
later (owner, 2026-11-11).

Per-tenant windows now, because they need a tenant settings table, an API
and a UI, where two server settings need none.

A `sensor_id` in the row, because a sensor has no identity
(`principles.md`, 3). An `api_key_id` in the row, because no caller asks which
key announced a scope and rows age out on their own. A key id can be added to
the row when a use for it appears.

An announce call beside sync, because sync already carries the tags.

## Consequences

The sensor API writes to the relational store (`adr/0001`). A rule is created
only after a sensor with its scope has synced, so a pipeline is deployed before
its rules are written, and a job that runs less often than the live window
cannot keep rules. A rule whose scope went stale stays in force, and the
management API flags it. The operator keeps the number of distinct tags low. The
caps bound the tags of each rule and of each sensor, and no cap bounds the tags
of a tenant. The server compares a rule's scope with the index in the tuple
form, which is the JSON that the engine writes for a scope map. The sync wire
(M7) carries the sensor's tags. The engine, the server and the web components
cite this record.
