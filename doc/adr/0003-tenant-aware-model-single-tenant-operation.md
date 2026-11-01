# 0003. Tenant-aware model, single-tenant operation

Date: 2026-09-21.

## Context

Retrofitting tenancy later would touch authentication and every entity.
Carrying a tenant id from the start costs almost nothing.

## Decision

Every entity, credential and event carries a tenant id, and every query is
tenant-scoped. The product ships one default tenant, with no tenant
administration, quotas or isolation guarantees.

## Rejected alternatives

No tenant id until a second tenant is needed, because of the retrofit cost.
Full multi-tenancy now, because nobody needs its administration, quotas or
isolation yet.

## Consequences

Every schema and every query carries the id. The store and management API
components cite this record.
