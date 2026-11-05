# 0025. Signed tokens and the revocation cutoff

Date: 2026-11-05.

## Context

People log in to the server from the browser UI, which the server renders
and which sends cookies, and from the CLI and the AI agent, which send
headers. The milestone names the login mechanism as forced. The owner chose
the token design, the key handling and the revocation rule while the login
unit was planned (owner, 2026-11-05).

## Decision

A logged-in caller proves who they are with an HS256 JWT the server signs
and does not store. The header carries `kid`, the id of the key that signed
it, and `typ`, the kind of token: `at+jwt` for an access token (RFC 9068)
and `tk-refresh+jwt`, a type of our own, for a refresh token, so one kind
never passes as the other. The claims are `sub`, the user id, `tid`, the
tenant id, `iat` with a microsecond fraction, and `exp`. An access token
lives one hour and is sent as `Authorization: Bearer`, or in the `tk_access`
cookie. A refresh token lives 7 days from login, is signed with the same
keys, and is sent in the body of the refresh route or in the `tk_refresh`
cookie, which goes to that route alone. Both cookies are `HttpOnly`,
`SameSite=Strict` and `Secure`. A header or body is read first, for the CLI
and the agent, and the cookie second, for the browser (owner, 2026-11-05).

A refresh returns a new access token and keeps the refresh token, so a login
ends 7 days after it began, however often it refreshes.

Every user row has `tokens_valid_after`. Every token of either kind is
checked against it on every request, so the server reads the user for each
request, and setting it to now revokes every token of the user. Logout sets
it, which ends every session of the user. A password change needs the
current password, sets it, clears the must-change flag, and hands the caller
fresh tokens, so the caller stays logged in while every other session ends
(owner, 2026-11-05). `iat` carries a microsecond fraction so that a token
issued in the same second as the cutoff compares exactly in both directions.

The signing keys are two passphrases typed by hand into the configuration,
each at least 32 characters: `auth.signing_key`, which is required and
signs, and `auth.previous_signing_key`, which is optional and only checks.
The names say what each key does, and a rotation reads as moving one to the
other. Neither has a command line flag, because `doc/adr/0013` warns on any
secret given as a flag (owner, 2026-11-05). The server stretches
each passphrase with Argon2id, at the parameters of `doc/adr/0022`, under a
random salt that `migrate` writes once into the `installation` table. The
salt gives every installation its own keys, and the stretching makes each
offline guess at a passphrase cost one Argon2id run, where an HS256
signature alone would cost one HMAC. A key's `kid` is a fingerprint of the
derived key, so nobody types an id, and a token is checked with the key its
`kid` names (owner, 2026-11-05).

Rotation: move `signing_key` to `previous_signing_key` and set a new
`signing_key` on every instance. After 7 days, the refresh token lifetime,
remove `previous_signing_key`.

The access token is checked by the server alone, so a symmetric key is
enough, and the caller's role comes from the stored user and not from the
token, so a role change takes effect at the next request.

## Rejected alternatives

A stored session token, because the owner chose a token the server does not
store. HTTP Basic, PASETO v4.local and a hand-rolled HMAC token, because
fewer tools read them than a JWT. An `aud` claim as the kind marker, because
the server is its only audience and `typ` is the standard explicit typing
(RFC 8725, 3.11). Integer seconds in `iat`, because a token from just before
a logout would survive up to a second (owner, 2026-11-05).

A map of key ids to secrets with an active-key setting, and a list of keys,
because two named keys make rotation two edits with nothing to name. Random
keys given as base64, because operators would need a tool to make them. A
fixed salt in the code, because one precomputed table would then work
against every installation, and a salt setting, because it is one more thing
to keep the same on every instance (owner, 2026-11-05).

Checking only refresh tokens against the cutoff, because revocation must take
effect at once. Stored opaque refresh tokens with rotation and reuse
detection, because they need a table and a second token style, and recorded
refresh JWTs, because they need both machineries. A new refresh token on
each refresh, because a login could then extend itself without limit (owner,
2026-11-05).

## Consequences

Every request with a token reads one user row. Revocation is per user, not
per session, so a logout ends every session of the user. There is no theft
detection: a stolen refresh token works until it expires or the user is
revoked. Every instance needs the same two passphrases, and one rotation is
in flight at a time. Rebuilding the database changes the salt and logs
everyone out. The `auth` module and the server component cite this record.
