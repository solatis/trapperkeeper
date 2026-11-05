# 0027. Opaque hashed API keys for sensors

Date: 2026-11-05.

## Context

Sensors authenticate to the sensor API with a long-lived credential, which
pipelines deploy with their code. The owner asked that the credentials of
people and of sensors be designed together, so the two designs fit, even
though the sensor keys arrive as code with later milestones (owner,
2026-11-05).

## Decision

A sensor API key is opaque random text of the form `tk_<key id>_<secret>`.
The server stores the key id, a SHA-256 of the secret, the tenant and the
scope tags, and never the secret. The sensor sends the key as a bearer
header over TLS. A key works until it is deleted (owner, 2026-11-05).

The signing keys of `doc/adr/0025` never touch sensor keys, so rotating the
people's keys does not break a deployed pipeline.

## Rejected alternatives

Long-lived JWTs on a separate signing ring, because a JWT cannot be revoked
per key and a key rotation breaks deployed pipelines. Request signing in the
style of SigV4, because the server would hold recoverable secrets and every
SDK would need canonical requests (owner, 2026-11-05).

## Consequences

Each sensor request costs one lookup by key id, which a cache can absorb. M4
manages the keys through the management API, and M5 checks them at the
sensor API. `doc/architecture.md` states the design under Sensor API and
Identity.
