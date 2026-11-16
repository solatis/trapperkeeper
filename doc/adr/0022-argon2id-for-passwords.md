# 0022. Argon2id for passwords

Date: 2026-11-06.

## Context

The server gets users with passwords. The superadmin, the user `admin` of the
system tenant `$tk` (`doc/adr/0028`), gets a generated password that reaches the
console once: the command that first migrates an empty database logs it as one
INFO event, and `reset-superadmin` prints a new one. The owner wanted stronger
parameters than the OWASP floor and asked for the trade-offs (owner,
2026-11-05).

## Decision

Passwords are hashed with Argon2id and stored as PHC strings, which carry
their own salt and parameters. The parameters are the second recommended
option of RFC 9106, section 4: 64 MiB of memory, 3 passes and 4 lanes
(owner, 2026-11-05).

At most as many hashes run at once as the host has CPUs, because login is
unauthenticated and each hash holds 64 MiB while it runs (owner, 2026-11-05).
Hashing runs on a blocking thread (`doc/adr/0014`).

A generated password is 16 random ASCII letters and digits, about 95 bits.
The server shows it once to the person who receives it (`doc/adr/0028`). The
server sets the user's must-change flag, so the UI asks for a new password at
the first login. The flag restricts nothing (owner, 2026-11-05).

## Rejected alternatives

The OWASP minimum of 19 MiB and 2 passes, because it has the least margin
against GPU guessing. The first recommended option of RFC 9106, 2 GiB per
hash, because it fills a small host with a few logins. bcrypt and scrypt,
because Argon2id is the current recommendation and resists both GPU and
side-channel attacks (owner, 2026-11-05).

Reading the superadmin's password from stdin or a no-echo prompt, because a
generated password that the server prints once works in a script, where a prompt
needs a person (owner, 2026-11-05).

## Consequences

Each login holds 64 MiB while it hashes. Under a flood of logins, the attempts
queue for a hashing permit, of which there are as many as the host has CPUs.
Raising the parameters later needs no schema change, because every stored hash
names its own. The superadmin's generated password passes through the console
or the log stream once. A user that an admin creates receives the password in
the response to the admin. The `auth` module cites this record.
