# 0025. Signed tokens and the revocation cutoff

Date: 2026-11-16.

## Context

People log in to the server from the browser UI, which the server renders
and which sends cookies, and from the CLI and the AI agent, which send
headers. The owner chose
the token design, the key handling and the revocation rule while the login
unit was planned (owner, 2026-11-05).

## Decision

A logged-in caller proves who they are with an HS256 JWT the server signs and
does not store. The header carries `kid`, the id of the key that signed the
token. The header also carries `typ`, the kind of token. An access token has the
`typ` `at+jwt` (RFC 9068). A refresh token has `tk-refresh+jwt`, a type of our
own. The two types differ, so a token of one kind never passes as the other. The
token carries four claims: `sub` holds the user id, `tid` the tenant id, `iat`
the issue time with a microsecond fraction, and `exp` the expiry. An access
token lives one hour and is sent as `Authorization: Bearer`, or in the
`tk_access` cookie. A refresh token lives 7 days from login, and the same keys
sign it. The caller sends it in the body of the refresh route or in the
`tk_refresh` cookie. The browser sends that cookie to the refresh route alone.
Both cookies are `HttpOnly`, `SameSite=Strict` and `Secure`. A header or body is
read first, for the CLI and the agent, and the cookie second, for the browser
(owner, 2026-11-05).
The `tk_access` cookie goes to `/api/v1` alone, the prefix of the routes that
read it (owner, 2026-11-16).

A refresh returns a new access token and keeps the refresh token, so a login
ends 7 days after it began, however often it refreshes.

Every user row has `tokens_valid_after`. The server checks every token of either
kind against `tokens_valid_after` on every request, so it reads the user for
each request. Setting `tokens_valid_after` to the current time revokes every
token of the user. Logout sets it, which ends every session of the user. A
password change needs the current password. It sets `tokens_valid_after` to the
current time and clears the must-change flag. It hands the caller fresh tokens,
so the caller stays logged in while every other session ends (owner,
2026-11-05). `iat` carries a microsecond fraction so that a token issued in the
same second as the cutoff fails when it was issued before the cutoff and passes
otherwise.

The signing keys are two passphrases typed by hand into the configuration, each
at least 32 characters: `auth.signing_key`, which is required and signs, and
`auth.previous_signing_key`, which is optional and only checks. The names say
what each key does, and a rotation reads as moving one to the other. Neither has
a command line flag, because `doc/adr/0013` warns on any secret given as a flag
(owner, 2026-11-05). The server stretches each passphrase with Argon2id, at the
parameters of `doc/adr/0022`, under a random salt. The step that applies the
migrations writes the salt once into the `installation` table, when the
installation has none, whether `serve` or `migrate` applies them. The salt gives
every installation its own keys. The stretching makes each offline guess at a
passphrase cost one Argon2id run, where an HS256 signature alone would cost one
HMAC. A key's `kid` is a fingerprint of the derived key, the first 8 bytes of
its SHA-256, so nobody types an id. The server checks a token with the key that
its `kid` names (owner, 2026-11-05).

To rotate, an operator moves `signing_key` to `previous_signing_key` and sets
a new `signing_key` on every instance. After 7 days, the refresh token
lifetime, the operator removes `previous_signing_key`.

Only the server checks an access token, so a symmetric key is enough. The
token carries no role, and the server reads the caller's role from the stored
user, so a role change takes effect at the next request.

## Rejected alternatives

A stored session token, because it needs a table of its own, a write at every
login and the cleanup of expired rows, and the owner prefers a stateless design
to a stateful one (owner, 2026-11-16). HTTP Basic, PASETO v4.local and a
hand-rolled HMAC token, because fewer tools read them than a JWT. An `aud` claim
as the kind marker, because the server is its only audience and `typ` is the
standard explicit typing (RFC 8725, 3.11). Integer seconds in `iat`, because a
token from just before a logout would survive up to a second (owner,
2026-11-05).

A map of key ids to secrets with an active-key setting, and a list of keys,
because two named keys make rotation two edits with nothing to name. Random
keys given as base64, because operators would need a tool to make them. A
fixed salt in the code, because one precomputed table would then work
against every installation, and a salt setting, because it is one more thing
to keep the same on every instance (owner, 2026-11-05).

Checking only refresh tokens against the cutoff, because revocation must take
effect at once. Stored opaque refresh tokens with rotation and reuse detection,
because they need a table and a second token style, and recorded refresh JWTs,
because they need both a table of tokens and signed JWTs. A new refresh token on
each refresh, because a login could then extend itself without limit (owner,
2026-11-05).

## Consequences

Every request with a token reads one user row. Because the cutoff is per user, a
user cannot end one session alone. The server detects no theft of a refresh
token, so a stolen refresh token works until it expires or until the user's
`tokens_valid_after` passes its issue time. Every instance needs the same two
passphrases. Only one rotation can be in progress at a time, because the
configuration holds two keys. Rebuilding the database changes the salt and logs
everyone out. The `auth` module and the server component cite this record.
