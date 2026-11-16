# 0024. Soft deletes with a deleted_at column

Date: 2026-11-15.

## Context

Users, roles and other entities will be deleted, and the events, audit trail
and reports that name them must keep working afterwards. The owner raised
the question while the users table was planned (owner, 2026-11-05).

## Decision

Every entity table has a `deleted_at` column, epoch microseconds in UTC like
every time column, and a deleted row stays in the table with it set. Every
read of a live entity filters on `deleted_at IS NULL` next to its `tenant_id`
filter (owner, 2026-11-05). The reads that resolve the user and the tenant of
an audit row do not filter, so the trail still names a user or a tenant that
was deleted later: `users::find_id_any` and the joins of `audit::page` (owner,
2026-11-15).

A name is unique among live rows only, so a deleted entity's name can be used
again. On SQLite and Postgres a partial unique index enforces this. MariaDB has
no partial index, so a unique key over a virtual generated `live` column
enforces it there (owner, 2026-11-05).

## Rejected alternatives

A live view per table, because every table then has two names and SQLite
views are read-only. Real deletes with snapshots of the deleted entity's
fields in every referencing row, because every report would need the copied
names (owner, 2026-11-05).

## Consequences

A forgotten filter is a silent bug, so tests include deleted rows. MariaDB
tables carry the generated `live` column. The store component cites this
record.
