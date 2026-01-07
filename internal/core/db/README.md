# Database Layer

Persistent storage layer supporting SQLite (development) and PostgreSQL (production) with embedded migrations, connection pooling, and named query management.

## Architecture

```
                    +------------------+
                    |   Application    |
                    +--------+---------+
                             |
              +--------------+--------------+
              |                             |
    +---------v---------+         +---------v---------+
    |      db.go        |         |   migrations.go   |
    | (sqlx connection) |         | (custom runner)   |
    +---------+---------+         +---------+---------+
              |                             |
              |    +-------------------+    |
              +--->|  queries/*.sql    |<---+
                   |  (dotsql named)   |
                   +-------------------+
                             |
              +--------------+--------------+
              |                             |
    +---------v---------+         +---------v---------+
    |      SQLite       |         |    PostgreSQL     |
    | (development)     |         |   (production)    |
    +-------------------+         +-------------------+
```

## Data Flow

```
Application Request
        |
        v
    db.Open() --> Connection Pool (sqlx)
        |              |
        |   +----------+----------+
        |   |                     |
        v   v                     v
    queries.Lookup()         db.Rebind()
        |                         |
        +------------+------------+
                     |
                     v
              sqlx.Get/Select/Exec
                     |
                     v
              Database Driver
```

## Why This Structure

The `internal/core/db/` package centralizes all database concerns:

- `db.go`: Single point for connection management; swapping database backends only affects this file
- `migrations.go`: Isolated migration logic; can be invoked from CLI without starting full application
- `queries/`: SQL files organized by entity; DBAs can review without Go knowledge

Separation from `internal/types/` is intentional: types are pure domain models with no database awareness. This enables SDK code to import types without pulling in database dependencies.

Migration embedding moved to `migrations/migrations.go` at repository root to avoid import cycles. The migrations package provides embed.FS instances consumed by `internal/core/db/migrations.go` migration runner.

## Invariants

1. All queries MUST include `WHERE deleted_at IS NULL` unless explicitly querying deleted records
2. All INSERT statements MUST set `created_at` and `modified_at` to current UTC time
3. All UPDATE statements MUST set `modified_at` to current UTC time
4. tenant_id MUST be set on all records even in single-tenant mode (use default tenant)
5. Migration checksums MUST match between binary and database; mismatch is fatal error
6. Connection pool MUST be closed on application shutdown; leaked connections exhaust server limits

## Tradeoffs

| Choice                   | Benefit                  | Cost                                                                 |
| ------------------------ | ------------------------ | -------------------------------------------------------------------- |
| CHAR(36) UUIDs           | Database portability     | 36 bytes vs 16 bytes for native UUID; index size increase            |
| TEXT timestamps (SQLite) | No custom type handling  | String comparison instead of native; application must enforce format |
| Soft deletes             | Audit trail preserved    | Queries more complex; storage grows indefinitely                     |
| Denormalized tenant_id   | Multi-tenancy ready      | Redundant data; must maintain consistency                            |
| Embedded migrations      | Single binary deployment | Larger binary; cannot hotfix schema without rebuild                  |

## Connection Pool Configuration

Pool limits prevent resource exhaustion:

- **MaxOpenConns: 16** - Based on PostgreSQL max_connections (100) divided by expected instances
- **MaxIdleConns: 4** - Balances resource usage vs connection establishment latency
- **ConnMaxIdleTime: 5 minutes** - Releases resources during quiet periods
- **ConnMaxLifetime: 30 minutes** - Prevents stale connections, forces periodic reconnection

These values match typical load balancer timeout defaults and prevent connection exhaustion under concurrent load.

## Migration Workflow

Migrations execute in deterministic order (001, 002, ...) with SHA256 checksum validation:

1. `MigrateUp()` detects database driver (sqlite3 or postgres)
2. Selects appropriate embedded migration directory from `migrations/` package
3. Validates checksums of previously applied migrations (detects tampering)
4. Applies pending migrations in transaction (atomic execution + recording)
5. Records metadata: migration_id, checksum, applied_at, execution_ms

Separate SQLite and PostgreSQL migration directories required because:

- SQLite uses TEXT timestamps; PostgreSQL uses TIMESTAMP WITHOUT TIME ZONE
- Different index syntax and constraint options
- Avoids runtime SQL parsing and conditional logic

## Query Management

Named queries in `queries/*.sql` files follow dotsql format:

```sql
-- name: get-tenant
SELECT tenant_id, name, created_at, modified_at, deleted_at
FROM tenants
WHERE tenant_id = ? AND deleted_at IS NULL;
```

Queries use `?` placeholders. sqlx Rebind() converts to `$1, $2` for PostgreSQL automatically. This keeps query files database-agnostic.

LoadQueries() walks embedded filesystem, combines all .sql files, and provides:

- `Exec(name, args)` - Execute named query
- `Get(name, dest, args)` - Single row into struct
- `Select(name, dest, args)` - Multiple rows into slice

DBA-friendly: SQL files can be reviewed and modified without Go knowledge. No code generation build step required (unlike sqlc).
