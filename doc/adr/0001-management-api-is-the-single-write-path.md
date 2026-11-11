# 0001. Management API is the single write path for configuration

Date: 2026-11-11.

## Context

The retired first attempt let the web UI write to the store directly. A CLI
and an AI agent were planned as further clients, and each would have needed
its own write path with its own validation and audit.

The sensor API receives events, and on every sync it sees the scope tags a
sensor declares. The seen-scope index (`adr/0034`) records those tags, so
the sensor API writes more than events.

## Decision

One management API owns every write of configuration: rules, API keys,
users and roles, the emergency pause. It validates, authorizes and records
the audit trail no matter who acts. The web UI, the CLI and the AI agent are
equal clients of it, and none of them touches the store.

The sensor API writes observations only: events, and the seen-scope index.
An observation is what a sensor reported or revealed about itself. It is
never configuration, and nothing a sensor sends changes what any rule does
(owner, 2026-11-11).

## Rejected alternatives

Clients writing to the store directly, because every new client would repeat
validation and audit, and the three would drift.

The seen-scope index maintained by the management API from events, because
it would lag behind the sensors and show only the ones that matched
something (owner, 2026-11-11).

## Consequences

A new client needs no new write path. The sensor API has two write paths of
its own, and neither leaves an audit row, because no principal acted. The
management API and sensor API components cite this record.
