# 0030. The superadmin mints a session for a user

Date: 2026-11-07.

## Context

M2 wants the superadmin to act as any user of any tenant. The ladder puts
the superadmin at the top (`adr/0029`), and the tokens a login hands out
exist (`adr/0025`).

## Decision

A superadmin route, `POST /api/v1/tenants/{tenant}/users/{name}/session`,
returns an ordinary access token and refresh token for the named user, as
a login body without cookies. The claims stay `sub`, `tid`, `iat` and
`exp`, and the client, a CLI or the web UI, decides how to use the tokens
(owner, 2026-11-07).

The route lives under `/tenants/{tenant}/users/{name}`, the one model every
tenant-scoped route follows (owner, 2026-11-07).

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
