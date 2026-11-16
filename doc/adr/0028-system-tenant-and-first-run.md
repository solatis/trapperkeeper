# 0028. The system tenant, the superadmin and first run

Date: 2026-11-16.

## Context

Tenants share one database (`adr/0003`), and someone has to create them. A
fresh install needs that person before anyone can log in, and the owner
wants one command to set up an install.

## Decision

The superadmin is the user `admin` of the system tenant `$tk`. `$tk` holds the
role `superadmin` and no other role, and no other tenant holds the role
`superadmin`. The superadmin logs in, receives tokens and is revoked by the
revocation cutoff (`adr/0025`) in the same way as every other user (owner,
2026-11-06).

Names that start with `$` are the server's own (owner, 2026-11-06). The API
refuses a tenant name or a user name that starts with `$`.

Every user is created with a generated password that must be changed at the
first login. Only the delivery differs: the console for the superadmin, or
the response to the person who created the user (owner, 2026-11-06).

The command that first migrates an empty database creates the superadmin and
logs its one-time password as one INFO event. serve does this on an empty
database, and migrate does it when an operator runs it first. serve refuses a
database that is not empty and has pending migrations (owner, 2026-11-06).

serve and migrate create the database that the URL names when it does not
exist yet, also on Postgres and MariaDB, so that one command sets up an
install (owner, 2026-11-16).

`reset-superadmin` recovers a lost password from the console. It gives the
superadmin a new one-time password and prints it. It creates the superadmin
when the superadmin is missing (owner, 2026-11-06).

## Rejected alternatives

A table of its own for the superadmin, because login would be duplicated,
and a flag on an ordinary user, because that user's tenant would become
special (owner, 2026-11-06).

Creating the superadmin in migrate only and never migrating in serve, because
the owner wants the first run to migrate and serve in one command. Migrating
on every serve, because rolling deploys rule it out (owner, 2026-11-06).

Several superadmins resetting each other's passwords, because they would need
routes to manage them. The project assumes a single superadmin for now. The
data model allows more superadmins, and support for them can be added later
(owner, 2026-11-06, 2026-11-16).

## Consequences

A mistyped SQLite path starts a new install. On Postgres and MariaDB the
database user of the first run needs the right to create databases. The
first password passes through the log stream once and must be changed at the
first login. Recovering the superadmin needs shell access to the install.
`$tk` is a tenant that is not a customer.
