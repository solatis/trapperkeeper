# server/src/auth/

**What**: authentication of people: password hashing and generated
passwords, the signed tokens and the keys that sign them, the login route,
the caller of a management route, and the role ladder that holds a route to
a minimum role.

**Map**:

| File          | What                                                                                                                                                   | When to read                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| `password.rs` | Argon2id hashing as PHC strings, generated passwords                                                                                                   | the hash parameters, the concurrency limit                                                 |
| `token.rs`    | the token kinds, their claims, the keyring: derive, issue, check                                                                                       | how a token is signed and checked, key rotation                                            |
| `login.rs`    | the login route, which takes a tenant name, a name and a password; the refresh, logout, password and me routes; the token cookies; the uniform refusal | what the routes hand out and refuse, the cookies' attributes, how a login names its tenant |
| `caller.rs`   | the `Caller` extractor and the token check behind it: key, kind, user read, cutoff                                                                     | what a management route checks before it runs                                              |
| `role.rs`     | the ladder and `AtLeast<R>`                                                                                                                            | which role a route needs, how a 403 comes about                                            |

**Invariants**:

- A password is stored only as its PHC string, which carries its own salt
  and parameters.
- Hashing runs on a blocking thread, and at most as many hashes run at once
  as the host has CPUs.
- HS256 is fixed by the server. A token is checked with the key its `kid`
  names and against the `typ` of its kind.
- Every request with a token, of either kind, reads the user and checks the
  token's issue time against the user's `tokens_valid_after`, so a logout or
  a password change takes effect at the next request.
- A route that needs a role takes `AtLeast<R>`. A route that serves the
  caller's own account takes `Caller`.
- A role that is not on the ladder ranks below every `AtLeast` route, so a
  role a tenant defines later opens no route until it gets a rung.
- A refresh hands out an access token and keeps the refresh token, so a
  login never extends past the refresh token's 7 days.

**Why**:

- Argon2id at the second recommended option of RFC 9106, with hashes limited
  to the CPU count (owner, 2026-11-05):
  `doc/adr/0022-argon2id-for-passwords.md`.
- Signed tokens the server does not store, two hand-typed signing keys
  stretched under a per-installation salt, and a per-user revocation cutoff
  (owner, 2026-11-05):
  `doc/adr/0025-signed-tokens-and-the-revocation-cutoff.md`.
