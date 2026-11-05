# server/src/auth/

**What**: authentication of people: password hashing and generated passwords.

**Map**:

| File          | What                                                 | When to read                               |
| ------------- | ---------------------------------------------------- | ------------------------------------------ |
| `password.rs` | Argon2id hashing as PHC strings, generated passwords | the hash parameters, the concurrency limit |

**Invariants**:

- A password is stored only as its PHC string, which carries its own salt
  and parameters.
- Hashing runs on a blocking thread, and at most as many hashes run at once
  as the host has CPUs.

**Why**:

- Argon2id at the second recommended option of RFC 9106, with hashes limited
  to the CPU count (owner, 2026-11-05):
  `doc/adr/0022-argon2id-for-passwords.md`.
- Signed tokens the server does not store, two hand-typed signing keys
  stretched under a per-installation salt, and a per-user revocation cutoff
  (owner, 2026-11-05):
  `doc/adr/0025-signed-tokens-and-the-revocation-cutoff.md`.
