# 0030. The superadmin mints a session for a user

Date: 2026-11-07.

## Context

The milestone M2 includes a superadmin who acts as any user of any tenant.
The ladder puts the superadmin at the top (`adr/0029`). Login already hands
out an access token and a refresh token (`adr/0025`).

## Decision

A superadmin route, `POST /api/v1/tenants/{tenant}/users/{name}/session`,
returns an ordinary access token and refresh token for the named user, as
a login body without cookies. The minted tokens carry the same claims as the
tokens of a login: `sub`, `tid`, `iat` and `exp`. The client, a CLI or the web
UI, decides how to use the tokens (owner, 2026-11-07).

Every tenant-scoped route lives under `/api/v1/tenants/{tenant}/...`, and the
mint route follows that pattern under `/tenants/{tenant}/users/{name}` (owner,
2026-11-07).

## Rejected alternatives

An actor claim naming the superadmin in the token, because it touches the
token, the caller and refresh, and adds a branch wherever the caller is read
(owner, 2026-11-07).

Setting the cookies as login does, because the superadmin's browser would
lose its own session.

## Consequences

A minted session is indistinguishable from the user's own in every route,
and only the audit trail shows who minted it. Logout from it ends all of
the user's sessions.
