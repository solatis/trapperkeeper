# ADR-010: Explicit Database Migrations

## Revision Log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

Database migrations need to be handled carefully in production environments. Automatic migrations can lead to:
- Unexpected downtime during deployments
- Data loss if migrations fail partway
- Inability to coordinate migrations across multiple instances
- Difficulty in rollback scenarios

We need a migration strategy that gives operators full control while providing integrity guarantees and proper failure recovery.

## Decision

Require explicit migration commands. The application will:
1. Check database version on startup
2. Refuse to start if migrations are needed
3. Provide clear error message with migration command
4. Track applied migrations with checksums for integrity validation
5. Validate checksums of previously applied migrations on startup

Migration files are embedded at compile time using the sqlx::migrate!() macro, ensuring migrations are always available with the binary.

### Migration Commands

Migration commands:
- `trapperkeeper-web --migrate` - Run pending migrations
- `trapperkeeper-api --migrate` - Run pending migrations

Both services use the sqlx::migrate!() macro to embed and execute migrations at compile time.

### Migration Tracking Schema

Migrations recorded in `migrations` table (schema details in Appendix):

The schema tracks:
- **migration_id**: Unique identifier from filename (e.g., "001_initial_schema")
- **checksum**: SHA256 hash of file content for integrity validation
- **applied_at**: Timestamp when migration was successfully applied
- **execution_ms**: How long the migration took to execute (for performance monitoring)

### Checksum Validation

On startup, the system validates checksums of all previously applied migrations:
- If a `migration_id` exists in the database with a different checksum than the embedded file, the system halts with an error
- This prevents applying modified migrations that could create inconsistent database states
- Operators must restore from backup if checksums mismatch

### Transaction Behavior

Each migration file is wrapped in a transaction on a best-effort basis:
- **PostgreSQL/MySQL**: Full transactional protection for most DDL operations
- **SQLite**: Limited protection due to implicit commits on some DDL operations

To ensure reliability:
- Migrations should be made idempotent using `IF NOT EXISTS` clauses
- Migrations should be kept small and atomic
- Migrations are never recorded as applied until fully successful

### Migration Failure Recovery

On migration failure, the recommended recovery approach is to restore from backup. The system does not attempt automatic rollback as this is considered over-engineered for the MVP.

This approach prioritizes simplicity and predictability:
- Operators understand exactly what happened (migration failed)
- Clear recovery path (restore from backup, investigate, fix)
- No complex rollback logic that could itself fail

### Multi-Database Support

Migrations are organized in database-specific directories (structure details in Appendix).

Migration files are embedded using the include_str! macro via sqlx, allowing for database-specific SQL while maintaining the same migration tracking mechanism across all database backends.

## Implementation

1. Add migration files to database-specific directories (e.g., `migrations/sqlite/`, `migrations/postgres/`, `migrations/mysql/`)
2. Use the `sqlx::migrate!()` macro to embed migrations at compile time in both `trapperkeeper-web` and `trapperkeeper-api` services
3. Configure the `migrations` tracking table schema in the initial migration file for each database backend
4. Implement startup logic to check database version and halt if migrations are pending
5. Provide clear error messages directing operators to run `--migrate` flag
6. Implement checksum validation on startup to detect modified migrations
7. Execute migrations transactionally where database backend supports it
8. Record migration success with `migration_id`, `checksum`, `applied_at`, and `execution_ms` in the tracking table

## Consequences

**Benefits:**
- Operators have full control over when migrations run
- Can review migration SQL before applying
- Easy to coordinate in multi-instance deployments
- Clear audit trail of applied migrations with timing data
- Prevents accidental schema changes
- Checksum validation prevents applying modified migrations
- Integrity protection ensures database consistency
- Execution timing helps identify performance issues
- Transaction wrapping provides safety where databases support it
- Compile-time embedding ensures migrations are always available with the binary

**Tradeoffs:**
- Extra deployment step required
- Application won't start without manual intervention
- Operators must remember to run migrations
- SQLite has limited transactional DDL protection
- Migration failures require backup restoration
- Multiple migration directories require maintenance

**Operational Implications:**
- Deployment procedure must include migration step before service restart
- Operators need access to run migration commands in production environments
- Downtime window required during migration execution
- Backup and restore procedures must be established before first production migration
- Multi-instance deployments require coordination to ensure only one instance runs migrations
- Monitoring should alert on pending migrations detected at startup

This approach prioritizes safety and control over convenience, which is appropriate for a system managing critical data pipeline rules.

## Related Decisions

**Extends:**
- **ADR-004: Database Backend** - Extends the multi-database strategy with explicit migration controls for SQLite, PostgreSQL, and MySQL

## Appendix A: Migration Schema and Structure

### Migrations Tracking Table

```sql
CREATE TABLE migrations (
    migration_id VARCHAR(128) PRIMARY KEY,  -- Filename without .sql extension
    checksum VARCHAR(64) NOT NULL,          -- SHA256 of file content
    applied_at TIMESTAMP NOT NULL,          -- When migration was applied
    execution_ms INTEGER                    -- Execution duration in milliseconds
);
```

### Directory Structure

```
migrations/
├── sqlite/
│   ├── 001_initial_schema.sql
│   └── 002_add_indices.sql
├── postgres/
│   ├── 001_initial_schema.sql
│   └── 002_add_indices.sql
└── mysql/
    ├── 001_initial_schema.sql
    └── 002_add_indices.sql
```
