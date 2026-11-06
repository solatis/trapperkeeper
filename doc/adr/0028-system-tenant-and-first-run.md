# 0028. The system tenant, the superadmin and first run

Date: 2026-11-06.

## Context

Tenants share one database (`adr/0003`), and someone has to create them. A
fresh install needs that person before anyone can log in, and the owner
wants one command to set up an install.

## Decision

The superadmin is the user `admin` of the system tenant `$tk`, which holds
the role `superadmin` alone. Login, tokens and the revocation cutoff serve
it unchanged (owner, 2026-11-06).

Names that start with `$` are the server's own (owner, 2026-11-06).

Every user is created with a generated password that must be changed at the
first login. Only the delivery differs: the console for the superadmin, or
the response to the person who created the user (owner, 2026-11-06).

The command that first migrates an empty database creates the superadmin and
logs its one-time password as one INFO event. serve does this on an empty
database, and migrate does it when an operator runs it first. A database
with pending migrations is still refused (owner, 2026-11-06).

`reset-superadmin` recovers a lost password from the console: it gives the
superadmin a new one-time password and prints it, and it creates the
superadmin when it is missing (owner, 2026-11-06).

## Rejected alternatives

A table of its own for the superadmin, because login would be duplicated,
and a flag on an ordinary user, because that user's tenant would become
special (owner, 2026-11-06).

Creating the superadmin in migrate only, migrating on every serve, which
rolling deploys rule out, and never migrating in serve (owner, 2026-11-06).

Several superadmins resetting each other's passwords (owner, 2026-11-06).

## Consequences

A mistyped SQLite path starts a new install. The first password passes
through the log stream once and must be changed at the first login.
Recovering the superadmin needs shell access to the install. `$tk` is a
tenant that is not a customer.
