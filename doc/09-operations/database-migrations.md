---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: database
hub_document: /Users/lmergen/git/trapperkeeper/doc/09-operations/README.md
tags:
  - migrations
  - database
  - golang-migrate
  - schema-management
---

# Database Migrations

## Context

Database migrations require careful handling in production environments to prevent unexpected downtime, data loss, and deployment coordination issues. This document specifies the explicit migration strategy requiring operator approval, migration tracking with checksums, and multi-database support.

**Hub Document**: This spoke is part of [Operations Overview](README.md). See the hub's Explicit Migration Strategy section for strategic context.

## Explicit Migration Commands

### Migration Philosophy

**Key Principle**: Require explicit operator approval for all database schema changes.

**Rationale**:

- Prevent unexpected downtime during deployments
- Prevent data loss if migrations fail partway
- Enable coordination across multiple instances
- Provide clear rollback capability

### Migration Subcommand

**Command Syntax**:

```bash
trapperkeeper migrate --db-url <url>
```

**Example Usage**:

```bash
# SQLite migration
trapperkeeper migrate --db-url "sqlite:///var/lib/trapperkeeper/trapperkeeper.db"

# PostgreSQL migration
trapperkeeper migrate --db-url "postgresql://user:pass@localhost/trapperkeeper"

# MySQL migration
trapperkeeper migrate --db-url "mysql://user:pass@localhost/trapperkeeper"
```

**Migration Process**:

1. Connect to database
2. Check `migrations` tracking table
3. Identify pending migrations
4. Execute pending migrations in order
5. Record each migration success with checksum and timing
6. Exit with success/failure status

### Service Startup Behavior

Services refuse to start if migrations pending:

```bash
# Attempt to start service with pending migrations
trapperkeeper sensor-api --db-url "postgresql://localhost/trapperkeeper"

# → Error output:
Error: Database schema out of date
  Current version: 002_add_indices
  Required version: 003_add_columns

  Run migrations:
    trapperkeeper migrate --db-url "postgresql://localhost/trapperkeeper"
```

**Startup Validation**:

1. Check database version on startup
2. Compare with expected version (embedded in binary)
3. If mismatch: Refuse to start with clear error message
4. If match: Continue with service initialization

**Operational Impact**: Application won't start without manual migration intervention, preventing silent misconfigurations.

## golang-migrate Migration Strategy

### Embedded Migrations Using embed.FS

Migrations embedded in binary using Go's `embed.FS`:

```go
package migrations

import (
    "embed"
    "io/fs"

    "github.com/golang-migrate/migrate/v4"
    "github.com/golang-migrate/migrate/v4/source/iofs"
)

// Embed migrations directory into binary
//go:embed sqlite/*.sql
var sqliteMigrations embed.FS

// Run migrations at runtime
func RunMigrations(dbURL string) error {
    // Create io/fs.FS from embed.FS
    sourceDriver, err := iofs.New(sqliteMigrations, "sqlite")
    if err != nil {
        return err
    }

    // Create migrator with embedded files
    m, err := migrate.NewWithSourceInstance("iofs", sourceDriver, dbURL)
    if err != nil {
        return err
    }

    // Run all pending migrations
    return m.Up()
}
```

**Benefits**:

- Migrations always available with binary (no external file dependencies)
- Version synchronization guaranteed (migrations + code released together)
- No migration file distribution required

**Build Requirement**: Migration files must be present at build time (committed to repository).

### Database-Specific Migration Directories

Migrations organized by database backend:

```
migrations/
├── sqlite/
│   ├── 001_initial_schema.sql
│   ├── 002_add_indices.sql
│   └── 003_add_columns.sql
├── postgres/
│   ├── 001_initial_schema.sql
│   ├── 002_add_indices.sql
│   └── 003_add_columns.sql
└── mysql/
    ├── 001_initial_schema.sql
    ├── 002_add_indices.sql
    └── 003_add_columns.sql
```

**Directory Selection**: Runtime selection of appropriate migration directory based on database connection URL.

**Example**:

```go
package migrations

import "embed"

//go:embed sqlite/*.sql
var sqliteMigrations embed.FS

//go:embed postgres/*.sql
var postgresMigrations embed.FS

//go:embed mysql/*.sql
var mysqlMigrations embed.FS

// SelectMigrations returns appropriate embed.FS based on database URL
func SelectMigrations(dbURL string) (embed.FS, string, error) {
    switch {
    case strings.HasPrefix(dbURL, "sqlite"):
        return sqliteMigrations, "sqlite", nil
    case strings.HasPrefix(dbURL, "postgres"):
        return postgresMigrations, "postgres", nil
    case strings.HasPrefix(dbURL, "mysql"):
        return mysqlMigrations, "mysql", nil
    default:
        return embed.FS{}, "", fmt.Errorf("unsupported database: %s", dbURL)
    }
}
```

**Cross-Reference**: See [Database Backend](database-backend.md) for database-specific SQL syntax differences.

## Migration Numbering and Ordering

### Numbering Convention

**Format**: `<number>_<description>.sql`

**Examples**:

- `001_initial_schema.sql`
- `002_add_indices.sql`
- `003_add_columns.sql`
- `004_add_rule_scope.sql`

**Ordering Rules**:

- Migrations applied in lexicographic order (001 before 002 before 003)
- Zero-padded numbers (001, 002, ..., 010, 011) for consistent sorting
- Same numbering across all database backends (sqlite/postgres/mysql)

### Migration Naming Convention

**Descriptive Names**: Use clear, action-oriented descriptions:

- `001_initial_schema.sql` (NOT `001_setup.sql`)
- `002_add_indices.sql` (NOT `002_optimize.sql`)
- `003_add_user_email.sql` (NOT `003_update_users.sql`)

**Naming Pattern**: `<action>_<subject>.sql`

- Actions: `add`, `create`, `drop`, `alter`, `rename`
- Subjects: Table names, column names, index names

### Migration Content Guidelines

**Single Responsibility**: Each migration should have one clear purpose.

**Good Example** (focused migration):

```sql
-- 002_add_indices.sql
CREATE INDEX idx_rules_tenant_deleted ON rules(tenant_id, deleted_at);
CREATE INDEX idx_users_tenant_email ON users(tenant_id, email);
```

**Bad Example** (mixed concerns):

```sql
-- 002_various_changes.sql (AVOID)
CREATE INDEX idx_rules_tenant_deleted ON rules(tenant_id, deleted_at);
ALTER TABLE users ADD COLUMN phone VARCHAR(20);
DROP TABLE old_sessions;
```

**Rationale**: Single-responsibility migrations easier to debug, rollback, and understand.

## Migration Tracking Table

### Schema Definition

The `migrations` table tracks applied migrations:

```sql
CREATE TABLE migrations (
    migration_id VARCHAR(128) PRIMARY KEY,  -- Filename without .sql extension
    checksum VARCHAR(64) NOT NULL,          -- SHA256 hash of file content
    applied_at TIMESTAMP NOT NULL,          -- When migration was applied
    execution_ms INTEGER                    -- Execution duration in milliseconds
);
```

**Field Descriptions**:

- **migration_id**: Unique identifier from filename (e.g., `001_initial_schema`)
- **checksum**: SHA256 hash of SQL file content for integrity validation
- **applied_at**: Timestamp when migration successfully completed
- **execution_ms**: Migration execution time for performance monitoring

**Example Records**:

```sql
INSERT INTO migrations VALUES
  ('001_initial_schema', 'a1b2c3...', '2025-01-01 10:00:00', 1234),
  ('002_add_indices', 'd4e5f6...', '2025-01-02 14:30:00', 567);
```

### Checksum Validation

On startup, services validate checksums of all previously applied migrations:

**Validation Process**:

1. Read embedded migration files (checksum calculated at runtime from embed.FS)
2. Query `migrations` table for applied migrations
3. Compare checksums for each migration_id
4. If mismatch: Halt with error

**Checksum Mismatch Error**:

```
Error: Migration checksum mismatch
  Migration: 001_initial_schema
  Expected checksum: a1b2c3d4e5f6...
  Actual checksum:   z9y8x7w6v5u4...

  This indicates the migration file was modified after being applied.
  To resolve:
    1. Restore from backup
    2. Investigate why migration file changed
    3. Never modify applied migrations
```

**Prevention**: Checksums prevent applying modified migrations that could create inconsistent database states.

**Rationale**: Modified migrations after application indicate serious integrity issue requiring operator intervention.

## Transaction Behavior

### Best-Effort Transaction Wrapping

Each migration file wrapped in transaction on best-effort basis:

**PostgreSQL/MySQL**:

```sql
-- Full transactional protection for most DDL
BEGIN;
  CREATE TABLE rules (...);
  CREATE INDEX idx_rules_tenant ON rules(tenant_id);
COMMIT;
```

**SQLite**:

```sql
-- Limited protection due to implicit commits on some DDL operations
BEGIN;
  CREATE TABLE rules (...);  -- Implicit COMMIT on schema change
  CREATE INDEX idx_rules_tenant ON rules(tenant_id);
COMMIT;
```

**Database Differences**:

- **PostgreSQL/MySQL**: Full transactional DDL support (rollback on error)
- **SQLite**: Implicit commits on schema changes (limited transactional protection)

### Idempotent Migration Pattern

**Recommendation**: Make migrations idempotent using `IF NOT EXISTS` clauses:

```sql
-- Idempotent table creation
CREATE TABLE IF NOT EXISTS rules (
    rule_id CHAR(36) PRIMARY KEY,
    name VARCHAR(128) NOT NULL
);

-- Idempotent index creation
CREATE INDEX IF NOT EXISTS idx_rules_tenant ON rules(tenant_id);

-- Idempotent column addition (PostgreSQL)
ALTER TABLE rules ADD COLUMN IF NOT EXISTS description TEXT;
```

**Benefits**:

- Safe to re-run migration after partial failure
- Reduces risk of manual database manipulation
- Simplifies recovery procedures

**Limitations**: Not all DDL supports `IF NOT EXISTS` (database-specific).

### Migration Failure Recovery

On migration failure, recommended recovery approach is to restore from backup:

**Recovery Procedure**:

1. Stop all services
2. Restore database from backup (taken before migration)
3. Investigate migration failure cause
4. Fix migration SQL (if needed)
5. Re-run migration: `trapperkeeper migrate --db-url <url>`
6. Restart services

**Rationale**: Restore-from-backup prioritizes simplicity and predictability over complex automatic rollback logic.

**No Automatic Rollback**: System does NOT attempt automatic rollback as this is considered over-engineered for MVP.

**Best Practice**: Always backup database before running migrations in production.

## Migration Recording

### Success Recording

Migrations recorded in `migrations` table ONLY after full success:

```go
// Migration execution pseudocode
func executeMigration(db *sql.DB, migration *Migration) error {
    startTime := time.Now()

    // Execute migration SQL
    _, err := db.Exec(migration.SQL)
    if err != nil {
        return err
    }

    executionMs := time.Since(startTime).Milliseconds()

    // Record success ONLY after SQL execution completes
    _, err = db.Exec(
        `INSERT INTO migrations (migration_id, checksum, applied_at, execution_ms)
         VALUES (?, ?, ?, ?)`,
        migration.ID,
        migration.Checksum,
        time.Now(),
        executionMs,
    )
    return err
}
```

**Key Principle**: Never record migration as applied until fully successful.

**Failure Handling**: If SQL execution fails, no record written to `migrations` table. Migration remains pending.

### Execution Timing

**execution_ms** field provides performance monitoring:

**Usage**:

- Identify slow migrations (helps plan maintenance windows)
- Track migration performance across database backends
- Detect performance regressions

**Example Query**:

```sql
-- Find slowest migrations
SELECT migration_id, execution_ms
FROM migrations
ORDER BY execution_ms DESC
LIMIT 10;
```

## Multi-Database Migration Support

### Shared Migration Logic, Database-Specific SQL

**Design Pattern**: Same migration numbering and logic, but database-specific SQL syntax.

#### Example: Adding Index

**SQLite** (`migrations/sqlite/002_add_indices.sql`):

```sql
CREATE INDEX idx_rules_tenant_deleted ON rules(tenant_id, deleted_at);
```

**PostgreSQL** (`migrations/postgres/002_add_indices.sql`):

```sql
CREATE INDEX CONCURRENTLY idx_rules_tenant_deleted ON rules(tenant_id, deleted_at);
```

**MySQL** (`migrations/mysql/002_add_indices.sql`):

```sql
CREATE INDEX idx_rules_tenant_deleted ON rules(tenant_id, deleted_at) ALGORITHM=INPLACE;
```

**Differences**:

- PostgreSQL: `CONCURRENTLY` for non-blocking index creation
- MySQL: `ALGORITHM=INPLACE` for online index creation
- SQLite: Standard syntax (no concurrent support)

**Cross-Reference**: See [Database Backend](database-backend.md) for complete database-specific SQL guidelines.

### Migration Tracking Across Databases

**Universal Tracking**: Same `migrations` table schema across all databases.

**Portability**: Migration records NOT portable across database backends (checksums differ for database-specific SQL).

**Migration Path** (SQLite → PostgreSQL):

1. Export data from SQLite
2. Create new PostgreSQL database
3. Run PostgreSQL migrations from scratch: `trapperkeeper migrate --db-url postgresql://...`
4. Import data into PostgreSQL
5. Update connection string in production

**No Direct Migration**: Cannot directly migrate database file from SQLite to PostgreSQL (schema differences, SQL dialect differences).

## Operational Deployment Procedure

### Pre-Migration Checklist

Before running migrations in production:

1. **Backup database**: Full backup with timestamp
2. **Review migration SQL**: Inspect all pending migrations
3. **Test on staging**: Run migrations on staging database first
4. **Plan downtime window**: Estimate downtime based on `execution_ms` from staging
5. **Coordinate with team**: Ensure all services stopped during migration

### Migration Execution

**Step-by-Step Procedure**:

```bash
# 1. Stop all services
systemctl stop trapperkeeper-sensor-api
systemctl stop trapperkeeper-web-ui

# 2. Backup database
pg_dump trapperkeeper > backup-$(date +%Y%m%d-%H%M%S).sql

# 3. Run migrations
trapperkeeper migrate --db-url "postgresql://localhost/trapperkeeper"

# 4. Verify migration success (check migrations table)
psql trapperkeeper -c "SELECT * FROM migrations ORDER BY applied_at DESC LIMIT 5;"

# 5. Start services
systemctl start trapperkeeper-sensor-api
systemctl start trapperkeeper-web-ui

# 6. Verify services healthy
curl http://localhost:50051/healthz  # sensor-api
curl http://localhost:8080/healthz   # web-ui
```

**Expected Output** (successful migration):

```
Applied 3 migrations:
  001_initial_schema (1234ms)
  002_add_indices (567ms)
  003_add_columns (234ms

Total execution time: 2035ms
```

### Multi-Instance Coordination

**Critical**: Only ONE instance should run migrations.

**Coordination Pattern**:

1. Stop all service instances
2. Run migrations from single admin node
3. Wait for migration completion
4. Start all service instances
5. Services verify schema version on startup

**Failure Mode**: If multiple instances run migrations concurrently, database locking prevents corruption (but causes migration failures).

## Edge Cases and Limitations

**Known Limitations**:

- **SQLite Transactional DDL**: Limited protection due to implicit commits
- **No Automatic Rollback**: Requires manual backup restoration
- **Downtime Required**: Brief downtime during migration execution
- **No Migration Splitting**: Cannot split single migration across multiple transactions

**Edge Cases**:

- **Checksum Mismatch**: Halt with error (requires manual investigation)
- **Migration Failure**: Migration NOT recorded as applied (safe to re-run)
- **Empty Migration File**: No-op (recorded as applied with 0ms execution time)
- **Duplicate Migration ID**: Error on insert into migrations table (duplicate primary key)

## Related Documents

**Dependencies** (read these first):

- [Operations Overview](README.md): Strategic context for explicit migration strategy
- [Database Backend](database-backend.md): Multi-database strategy, migration directory organization

**Related Spokes** (siblings in this hub):

- [Configuration Management](configuration.md): Database URL configuration for migrations

**Architecture References**:

- [Principles: Architectural Principles](../01-principles/README.md): MVP simplicity principle informing explicit migration approach
- [Architecture: Binary Distribution](../02-architecture/binary-distribution.md): Single binary with `migrate` subcommand
