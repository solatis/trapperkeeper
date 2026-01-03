---
doc_type: spoke
status: active
primary_category: validation
hub_document: doc/07-validation/README.md
tags:
  - database-validation
  - constraints
  - integrity
---

# Database Layer Validation

## Context

Database layer provides final enforcement through schema constraints, ensuring data integrity at storage level. Constraints complement application-layer validation with type safety, referential integrity, and uniqueness guarantees. No CHECK constraints due to backend variations (SQLite vs PostgreSQL).

**Hub Document**: This document is part of the Validation Architecture. See [Validation Hub](README.md) for complete validation strategy and layer distribution.

## Type Constraints

Column type constraints ensure data type safety.

### Column Types

```sql
CREATE TABLE rules (
    rule_id TEXT NOT NULL,           -- UUIDv7 as TEXT
    name TEXT NOT NULL,              -- Rule name
    sample_rate REAL NOT NULL DEFAULT 1.0,  -- 0.0-1.0 range
    created_at INTEGER NOT NULL,     -- Unix timestamp
    conditions TEXT NOT NULL         -- JSON/JSONB
);
```

**Type Mapping**:

- UUIDs: TEXT (UUIDv7 format)
- Timestamps: INTEGER (Unix epoch) or TIMESTAMP
- JSON data: TEXT (SQLite) or JSONB (PostgreSQL)
- Numeric: INTEGER, REAL
- Strings: TEXT

**Cross-References**:

- UUID Strategy: UUIDv7 format specification
- Timestamp Representation: Conversion strategy

## Foreign Key Constraints

Referential integrity between tables.

### Foreign Key Examples

```sql
CREATE TABLE rule_event_assignments (
    event_id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL,
    sensor_id TEXT NOT NULL,
    FOREIGN KEY (rule_id) REFERENCES rules(rule_id) ON DELETE CASCADE,
    FOREIGN KEY (sensor_id) REFERENCES sensors(sensor_id) ON DELETE CASCADE
);
```

**Cascade Behaviors**:

- `ON DELETE CASCADE`: Delete dependent records
- `ON DELETE SET NULL`: Nullify foreign key
- `ON DELETE RESTRICT`: Prevent deletion if dependencies exist

**Cross-References**:

- Database Backend: Multi-database support
- Database Migrations: Schema evolution

## Unique Indexes

Prevent duplicate identifiers.

### Unique Constraints

```sql
CREATE TABLE users (
    user_id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,  -- Prevent duplicate usernames
    email TEXT UNIQUE                -- Optional email uniqueness
);

CREATE UNIQUE INDEX idx_api_keys_id ON api_keys(api_key_id);
```

**Rationale**: Unique constraints at database level guarantee uniqueness even under concurrent writes. Application layer can pre-check but database provides final enforcement.

**Error Handling**: Unique constraint violations return user-friendly error "Resource already exists" (never expose raw SQL error).

**Cross-References**:

- Resilience Hub: Database error handling

## NOT NULL Constraints

Enforce required fields.

### NOT NULL Examples

```sql
CREATE TABLE rules (
    rule_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    description TEXT  -- Nullable (optional field)
);
```

**Rationale**: NOT NULL constraints at database level prevent null writes. Application layer validates before insertion but database provides final guarantee.

## No CHECK Constraints

CHECK constraints not used due to backend variations.

### Rationale

- **SQLite limitations**: CHECK constraints have syntax and feature differences from PostgreSQL
- **Backend flexibility**: Multi-database support (SQLite, PostgreSQL, MySQL) per architectural principles
- **Application-layer validation**: Range checks, enum validation, complex logic validated at API layer before persistence

**Alternative**: Application-layer validation before database writes ensures consistent behavior across all database backends.

**Cross-References**:

- Database Backend: Multi-database support rationale
- API Validation: Complete validation before persistence

## Migration Validation

Schema migration checksum validation ensures integrity.

### Checksum Verification

```sql
CREATE TABLE schema_migrations (
    version TEXT PRIMARY KEY,
    checksum TEXT NOT NULL,  -- SHA256 of migration file
    applied_at INTEGER NOT NULL
);
```

**Validation at startup**: Calculate SHA256 of migration file; compare against stored checksum; fail if mismatch detected.

**Rationale**: Prevents accidental modification of applied migrations. Ensures consistency across environments.

**Cross-References**:

- Database Migrations: Complete migration strategy
- Data Integrity Validation: Checksum validation specifications

## Error Handling

User-friendly error messages for database failures.

### Error Conversion

```go
import (
    "database/sql"
    "errors"
    "log/slog"
    "github.com/mattn/go-sqlite3"
)

_, err := db.Exec("INSERT INTO rules (name) VALUES (?)", name)
if err != nil {
    var sqliteErr sqlite3.Error
    if errors.As(err, &sqliteErr) && sqliteErr.ExtendedCode == sqlite3.ErrConstraintUnique {
        return ApiError{Type: "Conflict", Message: "Rule with this name already exists"}
    }
    slog.Error("Database error", "error", err)  // Log internal details
    return ApiError{Type: "Internal", Message: "Failed to create rule"}  // User-friendly message
}
```

**Never Expose**: Raw SQL errors, query text, internal database state, connection details.

**Cross-References**:

- Resilience Hub: Database error handling patterns

## Related Documents

**Dependencies**: Database Backend, Database Migrations, Validation Hub

**Related Spokes**:

- Responsibility Matrix: Complete Database Layer validation assignments for all 12 validation types (final enforcement)
- API Validation: Application-layer validation occurs before database validation
- Runtime Validation: Runtime validation complements database constraints
