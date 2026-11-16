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

A route names its lowest role as a type, `AtLeast<Admin>` or
`AtLeast<Superadmin>`. The superadmin needs no check of its own, because the
ladder ranks it at the top and only `$tk` holds the role (`adr/0028`).

A role a tenant defines later becomes a rung of the ladder plus permissions
(owner, 2026-11-06). Until it has a rung, a role that is not on the ladder
ranks below every route.

## Rejected alternatives

Permissions stored as rows, because they would need a table and seed rows before
any tenant defines a role of its own. A list of roles per route, because it
repeats role names and has no order (owner,
2026-11-06).

A separate superadmin check beside the ladder, because one mechanism covers
both.

## Consequences

A role off the ladder reaches only the routes of the caller's own account, such
as the password change. Every new route that needs a role states its rung in its
signature by taking `AtLeast<R>`. A route that serves the caller's own account
takes `Caller` and names no rung.
