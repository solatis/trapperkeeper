# 0001. Management API is the single write path

Date: 2026-09-21.

## Context

The retired first attempt let the web UI write to the store directly. A CLI and
an AI agent were planned as further clients, and each would have needed its own
write path with its own validation and audit.

## Decision

One management API owns every write that is not an event: rules, API keys,
users and roles, the emergency pause. It validates, authorizes and records the
audit trail no matter who acts. The web UI, the CLI and the AI agent are equal
clients of it, and none of them touches the store.

## Rejected alternatives

Clients writing to the store directly, because every new client would repeat
validation and audit, and the three would drift.

## Consequences

A new client needs no new write path. The management API component cites this
record.
