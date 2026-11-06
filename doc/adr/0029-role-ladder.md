# 0029. The role ladder

Date: 2026-11-06.

## Context

A tenant holds the roles admin, operator and observer, and the system tenant
holds the superadmin (`adr/0028`). Every management route has to decide which
of them may call it. A tenant may later define roles of its own
(`adr/0023`).

## Decision

The roles rank observer, operator, admin and superadmin, each above the one
before. Each route names the lowest role it accepts, and a caller below it
gets 403 (owner, 2026-11-06).

The superadmin sits at the top of the ladder and lives in `$tk` alone
(`adr/0028`), so its routes are a rung like the others.

A role a tenant defines later becomes a rung of the ladder plus permissions
(owner, 2026-11-06). Until it has a rung, a role that is not on the ladder
ranks below every route.

## Rejected alternatives

Permissions stored as rows, because they need a table and seeds now. A list
of roles per route, because it repeats role names and has no order (owner,
2026-11-06).

A separate superadmin check beside the ladder, because one mechanism covers
both.

## Consequences

A role off the ladder reaches only the routes of the caller's own account.
Every new route states its rung in its signature.
