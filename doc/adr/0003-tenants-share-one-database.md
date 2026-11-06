# 0003. Tenants share one database

Date: 2026-11-06.

## Context

TrapperKeeper runs as a hosted service for many customers and as a
self-hosted install, which usually holds one tenant. Retrofitting tenancy
later would touch authentication and every entity, and carrying a tenant id
from the start costs almost nothing.

## Decision

One binary serves a hosted install of many tenants and a self-hosted install
of usually one, with the same behavior (owner, 2026-11-06).

All tenants share one database. Every entity, credential and event carries a
tenant id, and every query is tenant-scoped (owner, 2026-11-06).

A person belongs to one tenant, and a login names the tenant together with
the name and the password (owner, 2026-11-06).

The superadmin creates tenants (`adr/0028`). Self-service signup comes later.

## Rejected alternatives

A mode setting, or a separate layer for the hosted service, because either
gives two behaviors to test (owner, 2026-11-06).

A database per tenant, because it needs migrations per tenant and a pool per
tenant (owner, 2026-11-06).

One person with memberships in several tenants, and an email unique across
the server, because a users table per tenant is simpler. The tenant in the
host name is left to a later UI (owner, 2026-11-06).

## Consequences

Every schema and every query carries the tenant id, and the tenant filter is
the only isolation between tenants. A person in two tenants has two
accounts. The store and management API components cite this record.
