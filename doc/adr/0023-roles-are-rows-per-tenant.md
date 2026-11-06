# 0023. Roles are rows per tenant

Date: 2026-11-05.

## Context

A user holds one of three roles: admin, operator and observer. The roles
could be a fixed value on each user or rows in a table of their own. The
owner decided while the first users were planned (owner, 2026-11-05).

## Decision

Roles are rows in a `roles` table, one set per tenant, each with a lowercase
`role_name` and a display `role_title`. The migration seeds the system tenant's
one role, `superadmin`, with a fixed id, a version 7 UUID with an all-zero
timestamp like the tenant's, so code and tests can name it (`doc/adr/0028`). Nearly
every table carries a tenant id, and the roles follow that rule (owner,
2026-11-05).

A user references its role together with its tenant, through a composite
foreign key on `(tenant_id, role_id)`, so a user cannot hold another
tenant's role. Code names roles by `role_name` and looks the id up in the
same statement, so callers never hold role ids.

## Rejected alternatives

A role column on `users` with a `CHECK` on the three names, because it leaves
no room for roles of a tenant's own and keeps the titles out of the store
(owner, 2026-11-05).

## Consequences

Reading a user joins `roles`, and every new tenant needs its roles seeded. A
role has a `deleted_at` like every entity (`doc/adr/0024`). The store
component cites this record.
