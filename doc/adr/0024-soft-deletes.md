# 0024. Soft deletes with a deleted_at column

Date: 2026-11-05.

## Context

Users, roles and other entities will be deleted, and the events, audit trail
and reports that name them must keep working afterwards. The owner raised
the question while the users table was planned (owner, 2026-11-05).

## Decision

Every entity table has a `deleted_at` column, epoch microseconds in UTC like
every time column, and a deleted row stays in the table with it set. Every
read filters on `deleted_at IS NULL` next to its `tenant_id` filter. A name
is unique among live rows only, so a deleted entity's name can be used again:
a partial unique index on SQLite and Postgres, and on MariaDB, which has no
partial index, a unique key over a virtual generated `live` column that is 1
for a live row and NULL for a deleted one, because a unique key treats NULLs
as distinct (owner, 2026-11-05).

## Rejected alternatives

A live view per table, because every table then has two names and SQLite
views are read-only. Real deletes with snapshots of the deleted entity's
fields in every referencing row, because every report would need the copied
names (owner, 2026-11-05).

## Consequences

A forgotten filter is a silent bug, so tests include deleted rows. MariaDB
tables carry the generated `live` column. Tenants got the column in their
first migration, edited in place, as `doc/principles.md` 8 allows. The store
component cites this record.
