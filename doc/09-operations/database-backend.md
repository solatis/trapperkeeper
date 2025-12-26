---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: database
hub_document: /Users/lmergen/git/trapperkeeper/doc/09-operations/README.md
tags:
  - database
  - sqlite
  - postgresql
  - mysql
  - dotsql
  - jmoiron-sqlx
  - connection-pooling
---

# Database Backend

## Context

TrapperKeeper requires persistent storage for relational data (rules, sensors, configurations) supporting multiple deployment scenarios: development and small deployments (easy setup), enterprise on-premise (existing database infrastructure), and multi-tenant cloud (production-grade database). This document specifies the multi-database backend strategy with SQLite, PostgreSQL, and MySQL support.

**Hub Document**: This spoke is part of [Operations Overview](README.md). See the hub's Multi-Database Backend Support section for strategic context.

## Multi-Database Support Philosophy

### Design Principle

Write lowest common denominator SQL that works across all three databases, avoiding database-specific features to maintain portability.

**Supported Databases**:

- **SQLite**: Zero-configuration default for development, evaluation, and small deployments
- **PostgreSQL**: Production deployments with full SQL feature set
- **MySQL**: Enterprise on-premise environments with existing MySQL infrastructure

**Key Constraint**: Cannot use database-specific features (PostgreSQL JSONB operators, MySQL-specific syntax, SQLite extensions).

### Database Selection Strategy

**Development and Evaluation**:

- Use SQLite (default)
- No external database required
- Single-file database (`{data-dir}/trapperkeeper.db`)
- Instant startup for demos

**Production Deployments**:

- Use PostgreSQL (recommended) or MySQL
- Connection pooling (16 connections per service instance)
- Native concurrent access
- Backup/replication infrastructure

**Migration Path**: Start with SQLite for evaluation, migrate to PostgreSQL/MySQL for production without code changes.

## SQLite as Default

### Zero-Configuration Startup

SQLite requires no external database process:

```bash
# Start service with SQLite (default)
trapperkeeper sensor-api --data-dir /var/lib/trapperkeeper
# → Uses /var/lib/trapperkeeper/trapperkeeper.db
```

**Benefits**:

- No installation required (embedded database)
- No configuration required (automatically created)
- Single binary deployment (no external dependencies)
- Perfect for evaluation and development

**Limitations**:

- Concurrent write limitations (mitigated by multiple writer support)
- No distributed deployment (single-file database)
- Limited connection pooling benefits (embedded database)

### SQLite Multiple Writer Support

Both `tk-sensor-api` and `tk-web-ui` share same SQLite database file with multiple writer support enabled:

```go
// SQLite connection configuration
import (
    "database/sql"
    "time"
    _ "github.com/mattn/go-sqlite3"
)

// Connection string with WAL and busy timeout
db, err := sql.Open("sqlite3",
    "/var/lib/trapperkeeper/trapperkeeper.db?_journal=WAL&_busy_timeout=5000&_synchronous=NORMAL")
if err != nil {
    return err
}

// Configure connection pool
db.SetMaxOpenConns(16)
db.SetMaxIdleConns(4)
db.SetConnMaxLifetime(30 * time.Minute)
```

**Concurrency Strategy**:

- Write-Ahead Logging (WAL) enables concurrent readers + single writer
- Busy timeout (5 seconds) prevents immediate lock errors
- Both services share database file safely

**Shared Access Example**:

```bash
# Terminal 1: Start sensor-api
trapperkeeper sensor-api --data-dir /var/lib/trapperkeeper --port 50051

# Terminal 2: Start web-ui (shares same database)
trapperkeeper web-ui --data-dir /var/lib/trapperkeeper --port 8080
```

**Result**: Both services access `/var/lib/trapperkeeper/trapperkeeper.db` concurrently without conflicts.

## PostgreSQL/MySQL for Production

### Connection String Format

**PostgreSQL**:

```bash
trapperkeeper sensor-api --db-url "postgresql://user:password@localhost:5432/trapperkeeper"
```

**MySQL**:

```bash
trapperkeeper sensor-api --db-url "mysql://user:password@localhost:3306/trapperkeeper"
```

**Connection String Components**:

- `user`: Database user with full privileges
- `password`: User password (use environment variables for secrets)
- `host`: Database server hostname or IP
- `port`: Database server port (5432 for PostgreSQL, 3306 for MySQL)
- `database`: Database name (`trapperkeeper`)

**Security Note**: Use environment variables for credentials:

```bash
export TK_DATABASE_URL="postgresql://user:password@localhost/trapperkeeper"
trapperkeeper sensor-api
```

**Cross-Reference**: See [Security: Configuration Security](../06-security/configuration-security.md) for complete secrets management.

### Database Driver Selection

Go's database/sql package supports all three databases through standard database drivers:

```go
import (
    "database/sql"

    _ "github.com/mattn/go-sqlite3"          // SQLite driver
    _ "github.com/lib/pq"                     // PostgreSQL driver
    _ "github.com/go-sql-driver/mysql"        // MySQL driver
)
```

**Runtime Selection**: Database driver selected at runtime based on connection string URL scheme:

- `sqlite3://` or `sqlite://` → SQLite driver (github.com/mattn/go-sqlite3)
- `postgresql://` or `postgres://` → PostgreSQL driver (github.com/lib/pq)
- `mysql://` → MySQL driver (github.com/go-sql-driver/mysql)

**Binary Size Impact**: All imported drivers are included in binary. For deployment-specific optimization, import only required drivers to reduce binary size.

### Schema Compatibility Across Databases

Lowest common denominator SQL ensures schema compatibility:

**Example Schema** (works across all databases):

```sql
-- Common SQL syntax supported by all three databases
CREATE TABLE rules (
    rule_id CHAR(36) PRIMARY KEY,  -- UUIDv7 as string
    tenant_id CHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL,
    modified_at TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP,

    INDEX idx_tenant_deleted (tenant_id, deleted_at)
);
```

**Avoided Database-Specific Features**:

- PostgreSQL UUID type (use CHAR(36) instead)
- PostgreSQL JSONB type (use TEXT with JSON serialization)
- MySQL AUTO_INCREMENT (use UUIDv7 for IDs)
- SQLite extensions (FTS, R\*Tree)

**Type Mapping**:

| Go Type          | SQLite                       | PostgreSQL            | MySQL        |
| ---------------- | ---------------------------- | --------------------- | ------------ |
| string           | TEXT                         | VARCHAR/TEXT          | VARCHAR/TEXT |
| int32/int64      | INTEGER                      | INTEGER/BIGINT        | INT/BIGINT   |
| bool             | INTEGER                      | BOOLEAN               | TINYINT(1)   |
| float32/float64  | REAL                         | REAL/DOUBLE PRECISION | FLOAT/DOUBLE |
| time.Time        | INTEGER (Unix epoch) or TEXT | TIMESTAMP             | TIMESTAMP    |
| UUID (as string) | CHAR(36)                     | CHAR(36)              | CHAR(36)     |

**Cross-Reference**: See [Data: Timestamp Representation](../03-data/timestamps.md) for complete timestamp handling across databases.

## Connection Pooling

### Pool Configuration

Connection pool configured per service instance:

```go
import (
    "database/sql"
    "time"
)

db, err := sql.Open("postgres", dbURL)
if err != nil {
    return err
}

// Configure connection pool
db.SetMaxOpenConns(16)                      // Maximum concurrent connections
db.SetMaxIdleConns(4)                       // Maximum idle connections
db.SetConnMaxIdleTime(5 * time.Minute)      // Idle connection timeout
db.SetConnMaxLifetime(30 * time.Minute)     // Connection max lifetime

// Verify connection with timeout context
ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
defer cancel()
if err := db.PingContext(ctx); err != nil {
    return err
}
```

**Default Configuration**:

- **Max open connections**: 16 per service instance
- **Max idle connections**: 4 (closed when idle timeout reached)
- **Idle timeout**: 5 minutes (close idle connections)
- **Max lifetime**: 30 minutes (recycle long-lived connections)

**Configuration Tuning**: Adjust based on deployment requirements:

- High-throughput: Increase max_connections (64+)
- Resource-constrained: Decrease max_connections (4-8)
- Database connection limits: Adjust to stay under database max_connections

**Example Configuration Override**:

```toml
# trapperkeeper.toml
[database]
max_connections = 32
connection_timeout_ms = 10000
idle_timeout_ms = 300000
```

### Connection Pool Sizing

**Per-Service Instance**:

- `tk-sensor-api`: 16 connections (handles concurrent gRPC requests)
- `tk-web-ui`: 16 connections (handles concurrent HTTP requests)

**Multi-Instance Deployments**:

- Total connections = instances × connections per instance
- Example: 3 sensor-api instances + 2 web-ui instances = (3 × 16) + (2 × 16) = 80 connections
- Ensure database max_connections > total required connections

**PostgreSQL Default**: 100 max_connections (adjust postgresql.conf if needed)

**MySQL Default**: 151 max_connections (adjust my.cnf if needed)

## Database-Specific Migrations

### Migration Organization

Migrations organized in database-specific directories:

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

**Design Principle**: Same migration numbering across databases, but database-specific SQL syntax.

### Example: Database-Specific Differences

**SQLite** (`migrations/sqlite/001_initial_schema.sql`):

```sql
CREATE TABLE rules (
    rule_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    created_at INTEGER NOT NULL  -- Unix timestamp (nanoseconds)
);
```

**PostgreSQL** (`migrations/postgres/001_initial_schema.sql`):

```sql
CREATE TABLE rules (
    rule_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL  -- Microsecond precision
);
```

**MySQL** (`migrations/mysql/001_initial_schema.sql`):

```sql
CREATE TABLE rules (
    rule_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    created_at TIMESTAMP(6) NOT NULL  -- Microsecond precision
);
```

**Migration Selection**: Migration tool automatically selects appropriate directory based on database URL at runtime.

**Cross-Reference**: See [Database Migrations](database-migrations.md) for complete migration strategy.

## SQL Query Layer with dotsql + jmoiron/sqlx

### Named Query Pattern

TrapperKeeper uses dotsql for named query management combined with jmoiron/sqlx for struct scanning:

```go
import (
    "database/sql"
    "github.com/gchaincl/dotsql"
    "github.com/jmoiron/sqlx"
)

// Load named queries from .sql file
dot, err := dotsql.LoadFromFile("queries/tenants.sql")
if err != nil {
    return err
}

// Execute named query with struct scanning
type Tenant struct {
    TenantID  string    `db:"tenant_id"`
    Name      string    `db:"name"`
    CreatedAt time.Time `db:"created_at"`
}

var tenant Tenant
query, err := dot.Raw("get-tenant")
if err != nil {
    return err
}
err = sqlxDB.Get(&tenant, query, tenantID)
```

**Named Query File** (`queries/tenants.sql`):

```sql
-- name: get-tenant
SELECT tenant_id, name, created_at
FROM tenants
WHERE tenant_id = ?;

-- name: list-tenants
SELECT tenant_id, name, created_at
FROM tenants
WHERE deleted_at IS NULL;
```

**Benefits**:

- Type safety enforced via struct tags
- Query validity verified at runtime (first execution)
- SQL queries in separate .sql files (not embedded in code)
- No build-time database connection required

### Dialect Differences Handling

Must handle minor SQL dialect differences:

**Placeholders**:

- SQLite/PostgreSQL: `?` or `$1, $2, $3`
- MySQL: `?`

**RETURNING Clause**:

- PostgreSQL: Supported (`INSERT ... RETURNING *`)
- SQLite: Supported (version 3.35+)
- MySQL: NOT supported (use `LAST_INSERT_ID()`)

**Example: Cross-Database Insert**:

```go
// PostgreSQL/SQLite with RETURNING
var ruleID string
query := "INSERT INTO rules (rule_id, name) VALUES (?, ?) RETURNING rule_id"
err := db.QueryRow(query, ruleID, name).Scan(&ruleID)
if err != nil {
    return err
}

// MySQL alternative (no RETURNING)
query := "INSERT INTO rules (rule_id, name) VALUES (?, ?)"
_, err := db.Exec(query, ruleID, name)
if err != nil {
    return err
}
// Use ruleID generated before insert
```

**Design Pattern**: Use explicit ID generation (UUIDv7) instead of database-generated IDs to avoid RETURNING clause dependency.

## Multi-Tenancy Database Design

### Schema Preparation

MVP supports single-tenant only in functionality, but database schemas prepared for multi-tenancy.

**Standard Audit Fields** (all tables):

- `<type>_id`: UUIDv7 primary key (e.g., `rule_id`, `user_id`, `tenant_id`)
- `tenant_id`: UUIDv7 foreign key (denormalized in all relevant tables)
- `created_at`: Timestamp, set once on creation
- `modified_at`: Timestamp, updated on every modification
- `deleted_at`: Timestamp, set on soft delete (NULL if not deleted)

**Example Table Schema**:

```sql
CREATE TABLE rules (
    rule_id CHAR(36) PRIMARY KEY,
    tenant_id CHAR(36) NOT NULL,
    name VARCHAR(128) NOT NULL,
    created_at TIMESTAMP NOT NULL,
    modified_at TIMESTAMP NOT NULL,
    deleted_at TIMESTAMP,

    INDEX idx_tenant_deleted (tenant_id, deleted_at)
);
```

**Cross-Reference**: See [Data: UUIDv7 Identifiers](../03-data/identifiers-uuidv7.md) for UUIDv7 identifier design.

### Soft Deletes

All tables use `deleted_at` for soft deletes:

**Soft Delete Pattern**:

```go
// Soft delete (set deleted_at)
now := time.Now()
_, err := db.Exec(
    "UPDATE rules SET deleted_at = ? WHERE rule_id = ?",
    now,
    ruleID,
)
if err != nil {
    return err
}
```

**Query Pattern** (exclude soft-deleted records):

```go
// Always filter deleted_at IS NULL
type Rule struct {
    RuleID    string     `db:"rule_id"`
    TenantID  string     `db:"tenant_id"`
    Name      string     `db:"name"`
    DeletedAt *time.Time `db:"deleted_at"`
}

var rules []Rule
err := sqlxDB.Select(&rules,
    "SELECT * FROM rules WHERE tenant_id = ? AND deleted_at IS NULL",
    tenantID,
)
if err != nil {
    return err
}
```

**Key Principles**:

- Soft delete is one-way (no undelete in MVP)
- Queries MUST filter `WHERE deleted_at IS NULL`
- Cascade soft deletes handled explicitly in application layer
- Add composite indexes: `(rule_id, deleted_at)` on child tables for performance

**Cross-Reference**: See [Data: Event Schema and Storage](../03-data/event-schema-storage.md) for complete soft delete cascading strategy.

## Related Documents

**Dependencies** (read these first):

- [Operations Overview](README.md): Strategic context for multi-database support
- [Database Migrations](database-migrations.md): Migration strategy and tracking

**Related Spokes** (siblings in this hub):

- [Configuration Management](configuration.md): Database URL configuration and environment variables

**Data References**:

- [Data: UUIDv7 Identifiers](../03-data/identifiers-uuidv7.md): UUIDv7 identifier design for all database IDs
- [Data: Timestamp Representation](../03-data/timestamps.md): Timestamp handling across databases
- [Data: Event Schema and Storage](../03-data/event-schema-storage.md): Soft delete strategy and cascading

**Security References**:

- [Security: Configuration Security](../06-security/configuration-security.md): Database credential management
- [Security: Encryption Strategy](../06-security/encryption.md): Application-layer encryption maintaining database backend flexibility
