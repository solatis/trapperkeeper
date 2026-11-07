# 0031. The audit trail

Date: 2026-11-07.

## Context

The management API owns the audit trail (`adr/0001`), and M2 wants every
changing request recorded where a tenant's admins can read it.

## Decision

One middleware records every non-GET request under `/api/v1` that matched a
route, after the handler answered: the time, the tenant acted on, the
caller, the method, the route template, the path parameters, the status,
the TCP peer and the client address. It never stores a body or a query
string. A login attempt is recorded with its status, under the tenant of
the user it named, and without the name (owner, 2026-11-06).

The client address is `X-Forwarded-For` as received when present, else the
peer. One proxy is assumed, and chains of proxies are out of scope (owner,
2026-11-07).

A failed write is logged at error and the response stands (owner,
2026-11-07).

A row is filed under the tenant the route resolved from its path when it has
one, and under the caller's tenant otherwise, so a tenant's admins see what
the superadmin did to their tenant, with the superadmin named as the caller
(owner, 2026-11-07).

Nothing prunes the table in M2 (owner, 2026-11-06).

## Rejected alternatives

Per-handler audit calls, because one middleware cannot be forgotten.

A client address derived behind configured trusted proxies, because it needs
a setting (owner, 2026-11-07). The peer alone, because behind a proxy every
row would show the proxy.

A 500 on a failed write, because the handler's change is already committed
and the client would retry it.

## Consequences

A row is written outside the handler's transaction, so a gap is possible,
and the error log marks it. A forged header on a server with no proxy is
recorded as the client, next to the true peer. The table grows without
bound until retention is decided (owner, 2026-11-07: a TTL later). The
superadmin's own trail in `$tk` holds only what it did outside a tenant's
path, such as creating tenants.
