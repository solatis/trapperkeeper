# ADR-010: Explicit Database Migrations

Date: 2025-10-28

## Related Decisions

**Depends on:**
- **ADR-004: Database Backend** - Extends the multi-database strategy with explicit migration controls for SQLite, PostgreSQL, and MySQL

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

### Migration Commands

Migration commands:
- `trapperkeeper-web --migrate` - Run pending migrations
- `trapperkeeper-api --migrate` - Run pending migrations

### Migration Tracking Schema

Migrations recorded in `migrations` table:

```sql
CREATE TABLE migrations (
    migration_id VARCHAR(128) PRIMARY KEY,  -- Filename without .sql extension
    checksum VARCHAR(64) NOT NULL,          -- SHA256 of file content
    applied_at TIMESTAMP NOT NULL,          -- When migration was applied
    execution_ms INTEGER                    -- Execution duration in milliseconds
);
```

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

Migrations are organized in database-specific directories as detailed in ADR-006:

```
internal/store/migrations/
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

This structure allows for database-specific SQL while maintaining the same migration tracking mechanism across all database backends.

## Consequences

**Pros:**
- Operators have full control over when migrations run
- Can review migration SQL before applying
- Easy to coordinate in multi-instance deployments
- Clear audit trail of applied migrations with timing data
- Prevents accidental schema changes
- Checksum validation prevents applying modified migrations
- Integrity protection ensures database consistency
- Execution timing helps identify performance issues
- Transaction wrapping provides safety where databases support it

**Cons:**
- Extra deployment step required
- Application won't start without manual intervention
- Operators must remember to run migrations
- SQLite has limited transactional DDL protection
- Migration failures require backup restoration
- Multiple migration directories require maintenance

This approach prioritizes safety and control over convenience, which is appropriate for a system managing critical data pipeline rules.
