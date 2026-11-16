# server/src/auth/

**What**: authentication of people: password hashing and generated
passwords, the signed tokens and the keys that sign them, the login, refresh,
logout, password and me routes, the `Caller` extractor, which authenticates
the caller of a management route, the role ladder, which ranks the roles, and
the `AtLeast<R>` extractor, which holds a route to a minimum role. Sensors
will identify themselves with API keys
(`doc/adr/0027-opaque-hashed-sensor-api-keys.md`), which no code handles yet.

**Map**:

| File          | What                                                                                                                                                                                                                                       | When to read                                                                                                                                                                                                                                                                       |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `mod.rs`      | the module header and the declarations of the five submodules                                                                                                                                                                              | adding a file to the module                                                                                                                                                                                                                                                        |
| `password.rs` | Argon2id hashing as PHC strings, generated passwords                                                                                                                                                                                       | the hash parameters, the concurrency limit                                                                                                                                                                                                                                         |
| `token.rs`    | the token kinds, their claims, the keyring: derive, issue, check                                                                                                                                                                           | how a token is signed and checked, key rotation                                                                                                                                                                                                                                    |
| `login.rs`    | the login, refresh, logout, password and me routes; the token cookies; the login body; the 401 without `WWW-Authenticate` that a failed login, a refresh without a token and a wrong current password get; the 500 for a store that failed | what the routes hand out and refuse, the cookies' attributes, how a login names its tenant, how a store failure becomes a 500 (`database_error`), the login body that `server/src/sessions.rs` reuses for the session that the superadmin mints for a user (`login_body`, `Login`) |
| `caller.rs`   | the `Caller` extractor and the token check behind it: key, kind, user read, and the user's `tokens_valid_after`                                                                                                                            | what a management route checks before it runs, the 401 and the 403 that a refused request gets                                                                                                                                                                                     |
| `role.rs`     | the ladder of the four roles, `AtLeast<R>` and the marker types `Admin` and `Superadmin`                                                                                                                                                   | which role a route needs, how a 403 comes about, what a role outside the four reaches                                                                                                                                                                                              |

**Invariants**:

- A password is stored only as its PHC string, which carries its own salt
  and parameters.
- Every request with a token, of either kind, reads the user and checks the
  token's issue time against the user's `tokens_valid_after`, so a logout or
  a password change takes effect at the next request.
- A route that needs a role takes `AtLeast<R>`. A route that serves the
  caller's own account takes `Caller`.

**Why**:

- Argon2id at the second recommended option of RFC 9106, with hashes limited
  to the CPU count (owner, 2026-11-05):
  `doc/adr/0022-argon2id-for-passwords.md`.
- Signed tokens the server does not store, and a per-user revocation cutoff.
  An operator types two passphrases into the configuration: a signing key,
  and an optional previous key that only checks. The server stretches each
  with Argon2id under the random salt that migrate writes once into the
  `installation` table (owner, 2026-11-05):
  `doc/adr/0025-signed-tokens-and-the-revocation-cutoff.md`.
- A minimum role per route on a ladder, with the superadmin at the top
  (owner, 2026-11-06): `doc/adr/0029-role-ladder.md`.
