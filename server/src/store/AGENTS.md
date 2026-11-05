# server/src/store/

**What**: the relational store, which holds the control-plane data in SQLite,
Postgres or MariaDB. The scheme of the database URL chooses the backend. The
event log is a separate store, whose engine is still open (see
`doc/decisions.md`).

**Map**:

| File                         | What                                                  | When to read                                                                     |
| ---------------------------- | ----------------------------------------------------- | -------------------------------------------------------------------------------- |
| `mod.rs`                     | choosing the backend, opening the store, transactions | which URL schemes work; why serve never creates the database; `Tx`; placeholders |
| `migrate.rs`                 | the embedded migrations: apply, check                 | what makes serve refuse, what each refusal means                                 |
| `users.rs`                   | creating a user with one of its tenant's roles        | the users table, the role subselect                                              |
| `../../migrations/sqlite/`   | the SQLite migration files, one per version           | adding a table                                                                   |
| `../../migrations/postgres/` | the Postgres migration files                          | adding a table                                                                   |
| `../../migrations/mariadb/`  | the MariaDB migration files                           | adding a table                                                                   |

**Invariants**:

- Every table other than `tenants` and `installation` has a `tenant_id`
  column, and every query filters on it (`doc/adr/0003`). `installation` is
  one row of facts that belong to no tenant.
- Every entity table has a `deleted_at` column, and a deleted row stays in the
  table. Every read filters on `deleted_at IS NULL` as it filters on
  `tenant_id`. A name is unique among live rows: a partial unique index on
  SQLite and Postgres, and a unique key over a generated `live` column on
  MariaDB, which has no partial index (`doc/adr/0024`).
- A reference to a tenant-scoped row carries the `tenant_id` too, as the
  composite key from `users` to `roles` does, so a row cannot reference
  another tenant's row.
- An entity's `_id` is a time-ordered UUID that Rust generates, stored as
  36-character text. Its `_name` is unique in its scope and lowercase, and
  constraints in the table enforce both. A display name is a `_title` column,
  which keeps its case (`doc/adr/0020`).
- Timestamps are integer epoch microseconds in UTC (`doc/adr/0019`).
- No query uses `RETURNING` or an upsert (`doc/adr/0019`).
- Every migration version exists in all three migration directories.
- Store functions take a `Tx` and never commit. The caller opens the
  transaction, composes the functions it needs, and commits (owner,
  2026-11-05).
- Queries use `$1` placeholders, each number once and in ascending order.
  `sql` in `mod.rs` rewrites them to `?` for MariaDB (`doc/adr/0019`).
- MariaDB tables declare `utf8mb4_bin`, so they compare text by bytes as the
  other two backends do.

**Why**:

- sqlx runs hand-written SQL, and each backend has its own checksummed
  migrations (owner, 2026-11-02):
  `doc/adr/0019-sqlx-with-hand-written-sql-over-three-backends.md`.
- Ids are internal and names are unique (owner, 2026-11-02):
  `doc/adr/0020-internal-ids-and-unique-names.md`.
- Roles are rows, seeded per tenant, because nearly every table carries a
  tenant id and a tenant may hold roles of its own (owner, 2026-11-05):
  `doc/adr/0023-roles-are-rows-per-tenant.md`.
- Entities are deleted softly, with a `deleted_at` column, so reports and
  references keep working after a delete (owner, 2026-11-05):
  `doc/adr/0024-soft-deletes.md`.
- serve opens the database without creating it, and only migrate creates it,
  so a mistyped path fails with "run migrate" instead of producing an empty
  database.
- migrate creates a missing database on every backend, so the first run is
  the same everywhere. On Postgres and MariaDB the migrate user therefore
  needs the right to create databases.

**How to**:

- Add a table: add the next numbered file to each of the three migration
  directories, with a `tenant_id` column, a `deleted_at` column, and the
  live-only unique index on the name as `0001_tenants.sql` has it.
- Start over after editing a migration: delete the database and run
  `migrate`.
- Run the tests against Postgres or MariaDB: set `TK_TEST_POSTGRES_URL` or
  `TK_TEST_MARIADB_URL` to the URL of a user allowed to create databases, such
  as `postgres://postgres:pw@127.0.0.1:5432/postgres`. Each test creates and
  drops databases named `tk_test_*`. `TK_TEST_EXPECT=postgres,mariadb` fails
  the run when either URL is missing, as CI sets it.
- Start local servers with the images CI uses, and remove them afterwards:

  ```sh
  docker run -d --rm --name tk-pg -e POSTGRES_PASSWORD=tk -p 55432:5432 postgres:latest
  docker run -d --rm --name tk-maria -e MARIADB_ROOT_PASSWORD=tk -p 53306:3306 mariadb:lts
  export TK_TEST_POSTGRES_URL=postgres://postgres:tk@127.0.0.1:55432/postgres
  export TK_TEST_MARIADB_URL=mariadb://root:tk@127.0.0.1:53306/mysql
  docker rm -f tk-pg tk-maria
  ```
