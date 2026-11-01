# 0004. Events embed rule snapshots

Date: 2026-09-21. Status: accepted.

## Context

Rules are versioned and retired, while events outlive them as audit records.
An event that only referenced its rule would depend on rule state with a
different retention.

## Decision

An event embeds each matched rule as it was evaluated. Events are
self-contained.

## Rejected alternatives

A reference to the rule version instead of a copy, because the event log then
could not have its own retention or engine.

## Consequences

Events are larger, which is acceptable since only matches are reported. The
event log can have its own retention and its own engine. The sensor API
component cites this record.
