# ADR-004: Database Backend Strategy

Date: 2025-10-28

## Related Decisions

**Depends on:**
- **ADR-001: Architectural Principles** - Implements MVP Simplicity principle with SQLite as zero-configuration default

**Extended by:**
- **ADR-010: Database Migrations** - Defines explicit migration strategy for database schema changes

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

Use standard `database/sql` with multiple driver support. Default to SQLite for zero-configuration startup. Write lowest common denominator SQL that works across all three databases.

### Database Drivers

- SQLite: `modernc.org/sqlite` (pure Go, no CGO)
- PostgreSQL: `github.com/lib/pq`
- MySQL: `github.com/go-sql-driver/mysql`

### Shared Access Pattern

Both tk-sensor-api and tk-web-ui share the same SQLite database file with multiple writer support enabled. SQLite's native multiple writer support handles concurrent access.

### Connection Pooling

Configuration per service:
- Max connections: 16
- Idle timeout: 5 minutes
- Connection lifetime: 30 minutes
- Use database/sql defaults in Go
- Configuration will be refined when adding PostgreSQL support

### Database Migrations

**Directory Structure:**
```
internal/store/
├── store.go           # Contains //go:embed directives
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

**Migration Tracking:**

Migrations recorded in `migrations` table:
- `migration_id` VARCHAR(128) PRIMARY KEY (filename without .sql)
- `checksum` VARCHAR(64) NOT NULL (SHA256 of file content)
- `applied_at` TIMESTAMP NOT NULL
- `execution_ms` INTEGER (execution duration)

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

Architecture using go:embed + sqlc:
- **sqlc**: Generate type-safe Go code from SQL queries
- **go:embed**: Embed SQL files directly in binary

**Directory Structure:**
```
internal/store/
├── store.go           # Contains //go:embed directives
├── sql/
│   ├── sqlite/
│   │   ├── tenants.sql
│   │   └── rules.sql
│   ├── postgres/
│   │   ├── tenants.sql
│   │   └── rules.sql
│   └── mysql/
│       ├── tenants.sql
│       └── rules.sql
```

Benefits: Type safety, no runtime SQL parsing, compile-time verification.

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

**API Keys Storage:**
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

**HMAC Secrets Storage:**
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

Note: HMAC secret management (environment variables, rotation, initialization) is covered separately in the API authentication specification. This ADR only covers the database schema.

**Sessions Storage:**
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

**Identifier Strategy:**

All identifiers use UUIDv7 format. See [ADR 007: UUID Strategy](007-uuid-strategy.md) for complete rationale and implementation details.

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

**Pros:**
- Zero dependencies beyond drivers
- SQLite default means instant startup for demos/development
- Enterprises can use their preferred database
- Direct SQL control and visibility
- Standard library approach
- Shared SQLite access simplifies deployment
- Type-safe SQL with sqlc
- Migration tracking prevents version mismatches
- Normalized schema prevents query performance issues with complex rules
- Multi-tenancy preparation enables future growth without major refactoring
- Soft deletes preserve audit history
- UUIDv7 identifiers provide natural time-ordering (see ADR 007)

**Cons:**
- Cannot use database-specific features (Postgres JSONB operators, etc.)
- Must handle minor dialect differences (placeholders, RETURNING clause)
- SQLite has concurrent write limitations (mitigated by multiple writer support)
- Manual SQL writing (no query builder)
- Normalized schema requires joins for rule retrieval (acceptable for read patterns)
- Soft delete cascades must be implemented manually in application layer
- Multiple migration directories require maintenance
- Migration failure recovery requires backup restoration

**Future Options:**
- If dialect differences become painful, implement separate Store implementations per database while keeping the same interface
- Add migration state tracking when operational complexity justifies it
- Optimize normalized schema query performance if needed
- Add automatic retention policies for soft-deleted records