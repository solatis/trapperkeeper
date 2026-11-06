# 0022. Argon2id for passwords

Date: 2026-11-05.

## Context

The server gets users with passwords, and the first admin gets a generated
password printed once. The owner wanted stronger parameters than the OWASP
floor and asked for the trade-offs (owner, 2026-11-05).

## Decision

Passwords are hashed with Argon2id and stored as PHC strings, which carry
their own salt and parameters. The parameters are the second recommended
option of RFC 9106, section 4: 64 MiB of memory, 3 passes and 4 lanes
(owner, 2026-11-05).

At most as many hashes run at once as the host has CPUs, because login is
unauthenticated and each hash holds 64 MiB while it runs (owner, 2026-11-05).
Hashing runs on a blocking thread (`doc/adr/0014`).

A generated password is 16 random ASCII letters and digits, about 95 bits,
shown once to the person who receives it (`doc/adr/0028`), and the user's
must-change flag is set so the UI asks for a new one at the first login. The flag restricts nothing (owner,
2026-11-05).

## Rejected alternatives

The OWASP minimum of 19 MiB and 2 passes, because it has the least margin
against GPU guessing. The first recommended option of RFC 9106, 2 GiB per
hash, because it fills a small host with a few logins. bcrypt and scrypt,
because Argon2id is the current recommendation and resists both GPU and
side-channel attacks (owner, 2026-11-05).

Reading the admin's password from stdin or a no-echo prompt, because a
generated password printed once is scriptable (owner, 2026-11-05).

## Consequences

Each login holds 64 MiB while it hashes, and logins queue under a flood.
Raising the parameters later needs no schema change, because every stored
hash names its own. A generated password shows in the terminal. The `auth`
module cites this record.
