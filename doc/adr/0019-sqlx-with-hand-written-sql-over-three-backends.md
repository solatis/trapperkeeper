# 0019. sqlx with hand-written SQL over SQLite, Postgres and MariaDB

Date: 2026-11-02.

## Context

The relational store holds the control-plane data: rules, keys, users, tenants
and the audit trail. The owner decided on 2026-11-02 that SQLite is the
embedded default and that Postgres and MariaDB are the server databases. All
three are supported and tested from the first table on.

## Decision

sqlx runs the store's SQL. Each statement is one hand-written SQL string,
checked at run time, and each table has a struct that derives `FromRow`
(owner, 2026-11-02).

One query set serves all three backends, so every query stays in the subset
the three share. The server generates ids in Rust, and no query uses
`RETURNING` or an upsert. Queries use numbered placeholders, which the store
rewrites to `?` for MariaDB. Timestamps are stored as integer epoch
microseconds in UTC, so the three backends bind them the same way (owner,
2026-11-02).

Each backend has its own migration directory of numbered SQL files. `migrate!`
embeds the files in the binary, and sqlx records each applied file with its
checksum in `_sqlx_migrations`. There are no down files, because deployments
are rebuilt rather than migrated backwards (owner, 2026-11-02;
`doc/principles.md` 8). `serve` refuses a database whose applied migrations do
not match the binary's.

The database setting is a URL in the form sqlx accepts as is, so the project
defines no connection format of its own (owner, 2026-11-02). The default is
`sqlite://trapperkeeper.db`, a file in the working directory, so a first run
needs no configuration.

## Rejected alternatives

sea-query and SeaORM, because each query takes 5 to 10 lines of Rust, they add
about 40 crates, each table is declared twice, and sea-query drops clauses
MariaDB lacks without an error. Diesel with diesel-async, because it has no
async multi-connection, reaches SQLite only through `spawn_blocking`, and
builds for one backend at a time. The sqlx `Any` driver, because it has no
uuid, time or json types and does not rewrite placeholders. The sqlx
compile-time macros, because they type each query to one backend and put
sqlx-cli and a `.sqlx` cache in the build loop.

One migration tree for all three backends, because MariaDB needs `VARCHAR`
keys and has no transactional DDL. One schema file with a hash, and numbered
files without checksums, because both let an edited file pass. refinery,
because it brings a second driver stack, and its rusqlite conflicts with sqlx
over the libsqlite3-sys link: sqlx-sqlite 0.9.0 needs a version below 0.38,
and rusqlite 0.41 needs 0.39.

MySQL as the third backend, because the owner chose MariaDB.

## Consequences

Nothing checks the SQL at compile time. SQLite binds NULL for a missing bind,
so the Postgres and MariaDB test runs are what catch arity and type mistakes.

The three migration directories change together, and CI on every backend
catches a directory that drifts. An edited migration that a database has
already applied makes `serve` and `migrate` refuse that database, and the
database is rebuilt.

SQLite is the default store, so it must build and pass on every platform CI
covers. A platform where it does not is a blocker for the owner to decide, and
the build never works around it (owner, 2026-11-02).

The store component cites this record.
