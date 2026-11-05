# 0026. Versioned API route prefix

Date: 2026-11-05.

## Context

The first management route arrived with login, and the owner asked where the
management routes live and whether their path carries a version (owner,
2026-11-05).

## Decision

Management routes live under `/api/v1`. The probes and the API document
stay at the root, where orchestrators and tools look for them (owner,
2026-11-05).

The `v1` reserves a namespace. Under `doc/principles.md` 8 a breaking change
replaces routes in place and does not add a `v2` beside `v1`.

## Rejected alternatives

`/api` without a version, and `/v1` without `api`, under which the API and
the UI's page paths would share the root (owner, 2026-11-05).

## Consequences

UI pages and the sensor API take paths outside `/api/v1`. The router nests
the management routes under the prefix, and the API document shows the
full paths. The server component cites this record.
