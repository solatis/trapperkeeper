# ADR-004: Database Backend Strategy

## Revision Log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper needs persistent storage for relational data (rules, sensors, configurations). Event data will be stored separately. We need to support multiple deployment scenarios:
- Development and small deployments (easy setup)
- Enterprise on-premise (often have existing database infrastructure)
- Multi-tenant cloud (need production-grade database)

Requirements:
- Support SQLite, PostgreSQL, and MySQL
- Zero-friction setup for evaluation
- No heavy ORM dependencies
- Direct SQL control
- Shared SQLite access for both tk-sensor-api and tk-web-ui services

## Decision

Use sqlx for database access with compile-time query verification. Default to SQLite for zero-configuration startup. Write lowest common denominator SQL that works across all three databases.

### Database Drivers

sqlx supports SQLite, PostgreSQL, and MySQL through a unified async API. Runtime feature flags control which database driver is compiled into the binary, reducing dependency footprint for specific deployments.

### Shared Access Pattern

Both tk-sensor-api and tk-web-ui share the same SQLite database file with multiple writer support enabled. SQLite's native multiple writer support handles concurrent access.

### Connection Pooling

Configuration per service:
- Max connections: 16
- Idle timeout: 5 minutes
- Connection lifetime: 30 minutes
- sqlx provides built-in async connection pooling
- Configuration will be refined when adding PostgreSQL support

### Database Migrations

Migrations are organized in database-specific directories (`sqlite/`, `postgres/`, `mysql/`) under `src/store/migrations/`. Each database has numbered migration files (e.g., `001_initial_schema.sql`, `002_add_indices.sql`). See [Appendix: Migration Structure](#appendix-migration-structure) for complete directory layout.

Migrations are embedded at compile time using the `sqlx::migrate!()` macro, which reads the appropriate database-specific migration directory and embeds all SQL files into the binary.

**Migration Tracking:**

Migrations recorded in `migrations` table with fields for migration_id, checksum, applied_at, and execution_ms. See [Appendix: Migrations Tracking Table](#appendix-migrations-tracking-table) for complete schema.

**Startup Behavior:**
- Service compares expected last migration with database
- Error on mismatch if database is behind
- Checksum validation halts with error if migration_id exists with mismatched checksum

**Transaction Wrapping:**
- Wrap each migration file in a transaction (best-effort)
- PostgreSQL/MySQL: Full transactional protection for most DDL
- SQLite: Limited protection due to implicit commits on some DDL operations
- Make migrations idempotent using `IF NOT EXISTS` clauses
- Keep migrations small and atomic
- Never record migration as applied until fully successful
- On failure: User must restore from backup

### SQL Query Layer

Architecture: Write SQL queries directly in Rust code using sqlx macros for compile-time verification. The `query!()` and `query_as!()` macros connect to a development database at compile time to verify queries are valid and extract result types.

See [Appendix: Query Example](#appendix-query-example) for a concrete Rust code example.

Benefits: Type safety enforced at compile time, compile-time verification ensures queries are valid and types match database schema, no separate code generation step required, direct mapping to Rust types.

### Multi-Tenancy Preparation

**MVP Scope:** Single-tenant only in functionality, but database schemas prepared for multi-tenancy.

**Schema Design Principles:**

All tables include standard audit fields:
- `<type>_id`: UUIDv7 primary key (e.g., `rule_id`, `user_id`, `tenant_id`)
- `tenant_id`: UUIDv7 foreign key (denormalized in all relevant tables)
- `created_at`: Timestamp, set once on creation
- `modified_at`: Timestamp, updated on every modification
- `deleted_at`: Timestamp, set on soft delete (NULL if not deleted)

**Critical Design Standard:** Schema column order should follow primary key column order whenever possible for query optimization.

**Soft Deletes:**
- All tables use `deleted_at` for soft deletes
- Soft delete is one-way (no undelete)
- Queries must filter: `WHERE deleted_at IS NULL`
- Cascade soft deletes handled explicitly in data management layer within a transaction using same timestamp
- Add composite indexes: `(rule_id, deleted_at)` on all child tables for performance

**Normalized Rule Storage:**

Rules stored in normalized relational structure across multiple tables:
- `rules`: Core rule fields (name, description, action, sample_rate, on_missing_field, enabled)
- `or_groups`: OR groups within rules (group_index for ordering)
- `conditions`: AND conditions within groups (field_type, op, value)
- `condition_fields`: Field paths as separate records (field_name/field_index, path_index)
- `rule_scope`: Rule scope tags (tag_key, tag_value pairs)

**API Keys and HMAC Secrets:**

API keys are stored with key hashes and references to HMAC secrets for authentication. HMAC secrets support primary/secondary key rotation. See [Appendix: API Keys and HMAC Secrets Tables](#appendix-api-keys-and-hmac-secrets-tables) for complete schemas.

Note: HMAC secret management (environment variables, rotation, initialization) is covered separately in the API authentication specification. This ADR only covers the database schema.

**Sessions Storage:**

Sessions table stores user session data with tenant_id, session_id, user_id, creation/expiration timestamps, and last activity tracking. See [Appendix: Sessions Table](#appendix-sessions-table) for complete schema.

**Identifier Strategy:**

All identifiers use UUIDv7 format. See [ADR-003: UUID Strategy](003-uuid-strategy.md) for complete rationale and implementation details.

**First Boot Initialization:**

Detection: Check if `tenants` table exists. Error if tables exist but are empty.

Table creation order (respects foreign key dependencies):
1. `tenants`
2. `teams` (references tenants)
3. `users` (references teams, tenants)
4. `hmac_secrets` (references tenants)
5. `api_keys` (references tenants, hmac_secrets)
6. `sessions` (references tenants, users)
7. `rules` (references tenants)
8. `rule_scope` (references rules)
9. `or_groups` (references rules)
10. `conditions` (references or_groups)
11. `condition_fields` (references conditions)

Default entities created on first boot:
- Default tenant
- Default team (schema preparation only, not MVP functionality)
- Default user with username="admin", password="admin", force_password_change=TRUE

## Consequences

**Benefits:**
- Minimal dependencies (sqlx and database drivers only)
- SQLite default means instant startup for demos/development
- Enterprises can use their preferred database
- Direct SQL control and visibility
- Compile-time query verification prevents SQL errors at runtime
- Async-first design integrates naturally with Tokio runtime
- Shared SQLite access simplifies deployment
- Type-safe SQL with compile-time verification
- Migration tracking prevents version mismatches
- Normalized schema prevents query performance issues with complex rules
- Multi-tenancy preparation enables future growth without major refactoring
- Soft deletes preserve audit history
- UUIDv7 identifiers provide natural time-ordering (see ADR 007)
- Direct mapping to Rust types eliminates serialization overhead

**Tradeoffs:**
- Cannot use database-specific features (Postgres JSONB operators, etc.)
- Must handle minor dialect differences (placeholders, RETURNING clause)
- SQLite has concurrent write limitations (mitigated by multiple writer support)
- Manual SQL writing (no query builder abstraction)
- Compile-time verification requires development database connection during build
- Normalized schema requires joins for rule retrieval (acceptable for read patterns)
- Soft delete cascades must be implemented manually in application layer
- Multiple migration directories require maintenance
- Migration failure recovery requires backup restoration

**Operational Implications:**
- Shared SQLite database file requires file system access coordination between tk-sensor-api and tk-web-ui services
- Connection pool configuration affects concurrent request handling capacity
- Migration version mismatches will halt service startup, requiring manual intervention
- Soft delete queries must explicitly filter `deleted_at IS NULL` in all application code
- Database file backups required before migration attempts for rollback capability

## Implementation

1. Configure sqlx with compile-time query verification using `query!()` and `query_as!()` macros
2. Organize migration files in database-specific directories (`sqlite/`, `postgres/`, `mysql/`) under `src/store/migrations/`
3. Embed migrations at compile time using `sqlx::migrate!()` macro
4. Implement migration tracking table with migration_id, checksum, applied_at, and execution_ms fields
5. Add startup validation to compare expected vs. actual migration state and halt on mismatch
6. Create initial database schemas with multi-tenancy support (tenant_id, created_at, modified_at, deleted_at columns)
7. Configure connection pooling with 16 max connections, 5-minute idle timeout, 30-minute connection lifetime
8. Enable SQLite multiple writer support for shared access between services
9. Implement first-boot initialization logic to create default tenant, team, and admin user

## Related Decisions

**Depends on:**
- **ADR-001: Architectural Principles** - Implements MVP Simplicity principle with SQLite as zero-configuration default

**Extended by:**
- **ADR-010: Database Migrations** - Defines explicit migration strategy for database schema changes

## Future Considerations

- If dialect differences become painful, implement separate Store implementations per database while keeping the same interface
- Add migration state tracking when operational complexity justifies it
- Optimize normalized schema query performance if needed
- Add automatic retention policies for soft-deleted records

## Appendix A: Migration Structure

Migration files are organized by database type:

```
src/store/
├── mod.rs            # Migration embedding using sqlx::migrate!()
├── migrations/
│   ├── sqlite/
│   │   ├── 001_initial_schema.sql
│   │   └── 002_add_indices.sql
│   ├── postgres/
│   │   ├── 001_initial_schema.sql
│   │   └── 002_add_indices.sql
│   └── mysql/
│       ├── 001_initial_schema.sql
│       └── 002_add_indices.sql
```

## Appendix B: Migrations Tracking Table

The `migrations` table tracks applied migrations:

```sql
migrations (
  migration_id VARCHAR(128) PRIMARY KEY,  -- filename without .sql
  checksum VARCHAR(64) NOT NULL,          -- SHA256 of file content
  applied_at TIMESTAMP NOT NULL,
  execution_ms INTEGER                    -- execution duration
)
```

## Appendix C: Query Example

Type-safe query example using sqlx macros:

```rust
let tenant = sqlx::query_as!(
    Tenant,
    "SELECT tenant_id, name, created_at FROM tenants WHERE tenant_id = ?",
    tenant_id
)
.fetch_one(&pool)
.await?;
```

## Appendix D: API Keys and HMAC Secrets Tables

**API Keys:**
```sql
api_keys (
  tenant_id UUID NOT NULL,
  api_key_id UUID NOT NULL,
  created_at, modified_at, deleted_at,
  key_name VARCHAR(128),
  key_hash_type VARCHAR(16) NOT NULL DEFAULT 'hmac-sha256',
  key_hash BINARY(32) NOT NULL,
  hmac_secret_id UUID,  -- FK to hmac_secrets
  last_used_at TIMESTAMP,
  PRIMARY KEY (tenant_id, api_key_id),
  INDEX idx_key_hash (key_hash)
);
```

**HMAC Secrets:**
```sql
hmac_secrets (
  tenant_id UUID NOT NULL,
  hmac_secret_id UUID NOT NULL,
  created_at, modified_at, deleted_at,
  secret BINARY(32) NOT NULL,
  is_primary BOOLEAN NOT NULL DEFAULT FALSE,
  PRIMARY KEY (tenant_id, hmac_secret_id)
);
```

## Appendix E: Sessions Table

```sql
sessions (
  tenant_id UUID NOT NULL,
  session_id UUID NOT NULL,
  user_id UUID NOT NULL,
  created_at,
  expires_at TIMESTAMP NOT NULL,
  last_activity_at TIMESTAMP NOT NULL,
  PRIMARY KEY (tenant_id, session_id)
);
```
