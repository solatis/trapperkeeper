# server/src/store/

**What**: the relational store, which holds the control-plane data in SQLite.
The event log is a separate store, whose engine is still open (see
`doc/decisions.md`).

**Map**:

| File                       | What                                        | When to read                     |
| -------------------------- | ------------------------------------------- | -------------------------------- |
| `mod.rs`                   | opening the store                           | how migrate creates the database |
| `migrate.rs`               | the embedded migrations and applying them   | what migrate applies and counts  |
| `../../migrations/sqlite/` | the SQLite migration files, one per version | adding a table                   |

**Invariants**:

- Every table other than `tenants` has a `tenant_id` column, and every query
  filters on it (`doc/adr/0003`).
- An entity's `_id` is a time-ordered UUID that Rust generates, stored as
  36-character text. Its `_name` is unique in its scope and lowercase, and
  constraints in the table enforce both. A display name is a `_title` column,
  which keeps its case (`doc/adr/0020`).
- Timestamps are integer epoch microseconds in UTC (`doc/adr/0019`).
- No query uses `RETURNING` or an upsert (`doc/adr/0019`).

**Why**:

- sqlx runs hand-written SQL, and each backend has its own checksummed
  migrations (owner, 2026-11-02):
  `doc/adr/0019-sqlx-with-hand-written-sql-over-three-backends.md`.
- Ids are internal and names are unique (owner, 2026-11-02):
  `doc/adr/0020-internal-ids-and-unique-names.md`.

**How to**:

- Add a table: add the next numbered file to the migration directory, with a
  `tenant_id` column and the unique constraint on the name.
- Start over after editing a migration: delete the database and run
  `migrate`.
