# server/src/auth/

**What**: authentication of people: password hashing and generated
passwords, the signed tokens and the keys that sign them, the login route,
and the caller of a management route.

**Map**:

| File          | What                                                               | When to read                                              |
| ------------- | ------------------------------------------------------------------ | --------------------------------------------------------- |
| `password.rs` | Argon2id hashing as PHC strings, generated passwords               | the hash parameters, the concurrency limit                |
| `token.rs`    | the token kinds, their claims, the keyring: derive, issue, check   | how a token is signed and checked, key rotation           |
| `login.rs`    | the login and me routes, the token cookie, the uniform refusal     | what login hands out and refuses, the cookie's attributes |
| `caller.rs`   | the `Caller` extractor: bearer header or cookie, user read, cutoff | what a management route checks before it runs             |

**Invariants**:

- A password is stored only as its PHC string, which carries its own salt
  and parameters.
- Hashing runs on a blocking thread, and at most as many hashes run at once
  as the host has CPUs.
- HS256 is fixed by the server. A token is checked with the key its `kid`
  names and against the `typ` of its kind.
- Every request with a token reads the user and checks the token's issue
  time against the user's `tokens_valid_after`, so a revocation takes effect
  at the next request.

**Why**:

- Argon2id at the second recommended option of RFC 9106, with hashes limited
  to the CPU count (owner, 2026-11-05):
  `doc/adr/0022-argon2id-for-passwords.md`.
- Signed tokens the server does not store, two hand-typed signing keys
  stretched under a per-installation salt, and a per-user revocation cutoff
  (owner, 2026-11-05):
  `doc/adr/0025-signed-tokens-and-the-revocation-cutoff.md`.
