# server/src/store/

**What**: this directory holds the relational store, and the store keeps the
control-plane data in SQLite, Postgres or MariaDB. That data is the tenants,
the roles, the users, the installation's facts and the audit trail. The
scheme of the database URL chooses the backend. The event log is a separate store, whose engine is
still open (see `doc/decisions.md`).

**Map**:

| File                         | What                                                                                                                                        | When to read                                                                                                           |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `audit.rs`                   | appending to and paging the audit trail                                                                                                     | the trail's columns, the cursor, the user join                                                                         |
| `mod.rs`                     | choosing the backend, opening the store, transactions                                                                                       | which URL schemes work; whether a database exists; creating a missing database; `Tx`; placeholders; the timestamp form |
| `installation.rs`            | the installation's facts that belong to no tenant: the salt with which the server stretches its token signing keys                          | the signing salt, the installation table                                                                               |
| `migrate.rs`                 | the embedded migrations: apply, check                                                                                                       | what makes serve refuse, what each refusal means, what an empty database is                                            |
| `tenants.rs`                 | tenants and their roles; finding a tenant by name; whether a tenant holds a role                                                            | creating a tenant with its roles, listing tenants                                                                      |
| `users.rs`                   | creating, listing, finding, re-roling and deleting a user; counting a tenant's live admins; revoking a user's tokens and setting a password | the users table, the role subselect, the soft delete                                                                   |
| `../../migrations/sqlite/`   | the SQLite migration files, one per version                                                                                                 | adding a table; what each column means                                                                                 |
| `../../migrations/postgres/` | the Postgres migration files                                                                                                                | adding a table                                                                                                         |
| `../../migrations/mariadb/`  | the MariaDB migration files                                                                                                                 | adding a table; the MariaDB forms: VARCHAR columns, the `utf8mb4_bin` collation, the generated `live` column           |

**Invariants**:

- The migrations seed one tenant, the system tenant `$tk`. `$tk` holds the
  role `superadmin` and no other role (`doc/adr/0028`).
- Every table other than `tenants` and `installation` has a `tenant_id`
  column, and every query filters on it (`doc/adr/0003`). `installation` is
  one row of facts that belong to no tenant.
- Every entity table has a `deleted_at` column. A deleted row stays in its
  table with `deleted_at` set. `audit` holds records, which nothing deletes,
  so it has no `deleted_at`. Every read of a live entity filters on
  `deleted_at IS NULL`. The reads that resolve the user and the tenant of an
  audit row do not filter, so the trail still names a user or a tenant that
  was deleted later: `users::find_id_any` and the joins of `audit::page`
  (owner, 2026-11-15). A name is unique among live rows. On SQLite and
  Postgres a partial unique index enforces that. MariaDB has no partial
  index, so a unique key over a generated `live` column enforces it there
  (`doc/adr/0024`).
- A reference to a tenant-scoped row carries the `tenant_id` too, as the
  composite key from `users` to `roles` does, so a row cannot reference
  another tenant's row. The exception is an actor reference, which names the
  user who made a request. An actor reference is a foreign key on
  `users (user_id)` alone, because the actor may be the superadmin of `$tk`
  acting on a customer tenant. `audit.user_id` is an actor reference (owner,
  2026-11-07).
- The server generates an entity's `_id`, a version 7 UUID, which is stored
  as 36-character text. Its `_name` is lowercase and unique in its scope,
  which is the installation for a tenant and the tenant for a role or a
  user. Constraints in the table enforce the lowercase and the uniqueness. A display name is a `_title` column,
  which keeps its case (`doc/adr/0020`).
- Timestamps are integer epoch microseconds in UTC (`doc/adr/0019`).
- No query uses `RETURNING` or an upsert (`doc/adr/0019`).
- Every migration version exists in all three migration directories
  (`doc/adr/0019`).
- Store functions take a `Tx` and never commit. The caller opens the
  transaction, composes the functions it needs, and commits (owner,
  2026-11-05).
- Queries use `$1` placeholders, each number once and in ascending order,
  because MariaDB's `?` binds by position, and `sql` in `mod.rs` drops the
  numbers when it rewrites the placeholders to `?` (`doc/adr/0019`).
- MariaDB tables declare `utf8mb4_bin`, so MariaDB compares text by bytes, as
  Postgres and SQLite do for equality. The Postgres migrations declare no
  collation, so Postgres orders text by the collation of the database and
  compares by bytes only for equality (`doc/adr/0020`).

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
- serve and migrate create a missing database on every backend, so the
  first run is the same everywhere. On Postgres and MariaDB the user of the
  first run therefore needs the right to create databases.

**How to**:

- Add a table: add the next numbered file to each of the three migration
  directories, with a `tenant_id` column, a `deleted_at` column, a lowercase
  CHECK on the name, and the live-only unique index on
  `(tenant_id, <entity>_name)`, as `0002_roles.sql` and `0003_users.sql` have
  them. On MariaDB the table also declares the `utf8mb4_bin` collation and
  gets the `live` column. A table of records rather than entities, as
  `audit` is, has no `deleted_at`.
- Start over after editing a migration: delete the database and run
  `migrate`.
- Run the tests against Postgres or MariaDB: set `TK_TEST_POSTGRES_URL` or
  `TK_TEST_MARIADB_URL` to the URL of a user allowed to create and drop
  databases. Each test creates and drops databases named `tk_test_*`. A run
  whose `TK_TEST_EXPECT` names a backend fails when that backend's URL is not
  set. CI sets `TK_TEST_EXPECT` on each runner to the backends the runner
  starts.
- Start local servers with the images CI uses:

  ```sh
  docker run -d --rm --name tk-pg -e POSTGRES_PASSWORD=tk -p 55432:5432 postgres:latest
  docker run -d --rm --name tk-maria -e MARIADB_ROOT_PASSWORD=tk -p 53306:3306 mariadb:lts
  export TK_TEST_POSTGRES_URL=postgres://postgres:tk@127.0.0.1:55432/postgres
  export TK_TEST_MARIADB_URL=mariadb://root:tk@127.0.0.1:53306/mysql
  ```

  Remove the servers when the tests are done:

  ```sh
  docker rm -f tk-pg tk-maria
  ```
