# server/src/store/

**What**: the relational store, which holds the control-plane data in SQLite,
Postgres or MariaDB. The scheme of the database URL chooses the backend. The
event log is a separate store, whose engine is still open (see
`doc/decisions.md`).

**Map**:

| File                         | What                                                         | When to read                                                                      |
| ---------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| `mod.rs`                     | choosing the backend, opening the store for serve or migrate | which URL schemes work; why serve never creates the database; the readiness probe |
| `migrate.rs`                 | the embedded migrations: apply, check                        | what makes serve refuse, what each refusal means                                  |
| `../../migrations/sqlite/`   | the SQLite migration files, one per version                  | adding a table                                                                    |
| `../../migrations/postgres/` | the Postgres migration files                                 | adding a table                                                                    |
| `../../migrations/mariadb/`  | the MariaDB migration files                                  | adding a table                                                                    |

**Invariants**:

- Every table other than `tenants` has a `tenant_id` column, and every query
  filters on it (`doc/adr/0003`).
- An entity's `_id` is a time-ordered UUID that Rust generates, stored as
  36-character text. Its `_name` is unique in its scope and lowercase, and
  constraints in the table enforce both. A display name is a `_title` column,
  which keeps its case (`doc/adr/0020`).
- Timestamps are integer epoch microseconds in UTC (`doc/adr/0019`).
- No query uses `RETURNING` or an upsert (`doc/adr/0019`).
- Every migration version exists in all three migration directories.
- Queries use `$1` placeholders. MariaDB's `?` comes from a rewrite in the
  store, which arrives with the first parameterized query (`doc/adr/0019`).
- MariaDB tables declare `utf8mb4_bin`, so they compare text by bytes as the
  other two backends do.

**Why**:

- sqlx runs hand-written SQL, and each backend has its own checksummed
  migrations (owner, 2026-11-02):
  `doc/adr/0019-sqlx-with-hand-written-sql-over-three-backends.md`.
- Ids are internal and names are unique (owner, 2026-11-02):
  `doc/adr/0020-internal-ids-and-unique-names.md`.
- serve opens the database without creating it, and only migrate creates it,
  so a mistyped path fails with "run migrate" instead of producing an empty
  database.
- migrate creates a missing database on every backend, so the first run is
  the same everywhere. On Postgres and MariaDB the migrate user therefore
  needs the right to create databases.

**How to**:

- Add a table: add the next numbered file to each of the three migration
  directories, with a `tenant_id` column and the unique constraint on the
  name.
- Start over after editing a migration: delete the database and run
  `migrate`.
