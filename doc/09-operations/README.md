---
doc_type: hub
status: active
primary_category: deployment
consolidated_spokes:
  - configuration.md
  - database-backend.md
  - database-migrations.md
  - cli-design.md
  - web-framework.md
  - health-endpoints.md
tags:
  - operations
  - deployment
  - configuration
  - database
---

# Operations Overview

## Context

TrapperKeeper's operational architecture must support rapid deployment with zero-friction evaluation (SQLite default), production-grade backends (PostgreSQL/MySQL), and clear configuration management across file/environment/CLI sources. A five-engineer team requires operational simplicity without sacrificing production requirements.

Traditional approaches create friction: ORMs introduce complexity, automatic migrations risk data loss, multi-binary deployments increase coordination overhead. TrapperKeeper needs operational patterns balancing developer velocity with production safety: explicit migrations, multi-source configuration with clear precedence, unified CLI, and framework selections optimizing for small teams.

This hub consolidates operational decisions establishing configuration strategy, database backend support, migration approach, CLI design, web framework selection, and health check patterns.

## Decision

We implement **explicit operational controls** with multi-source configuration (file/env/CLI), multi-database support (SQLite/PostgreSQL/MySQL), explicit migrations requiring operator approval, cobra-based CLI with subcommands, net/http web framework, and standardized health endpoints.

This document serves as the operations hub providing strategic overview with cross-references to detailed implementation documents for configuration, databases, migrations, CLI, web framework, and health checks.

### Multi-Source Configuration

Configuration loaded from three sources with explicit precedence: CLI arguments > Environment variables > Configuration files > Defaults.

**Configuration Sources**:

- **Configuration files** (TOML): `/etc/trapperkeeper/trapperkeeper.toml`, `~/.config/trapperkeeper/trapperkeeper.toml`, `./trapperkeeper.toml`
- **Environment variables**: `TK_*` prefix (e.g., `TK_SENSOR_API_PORT=50051`)
- **CLI arguments**: `--port`, `--data-dir`, `--db-url`

**Precedence Pipeline**: CLI > Env > File > Defaults (highest priority wins)

**Key Principles**:

- Clear hierarchy prevents configuration ambiguity
- Secrets restricted to environment variables and CLI (never files)
- Validation at startup catches misconfigurations early
- viper library provides unified multi-source loading

**Cross-References**:

- Configuration Management: Complete specification of formats, precedence, validation
- CLI Design: CLI argument parsing with cobra
- Security: Configuration Security: Secrets enforcement policies

**Example**:

```toml
# trapperkeeper.toml
[sensor_api]
port = 50051
max_connections = 1000
```

```bash
export TK_HMAC_SECRET="secret-key"  # Secrets via env only
./trapperkeeper sensor-api --port 9090  # CLI overrides file
# Result: port=9090 (CLI), max_connections=1000 (file), hmac_secret from env
```

### Multi-Database Backend Support

SQLite default for zero-configuration, PostgreSQL/MySQL for production, with lowest-common-denominator SQL.

**Supported Databases**:

- **SQLite**: Zero-configuration default, development and small deployments
- **PostgreSQL**: Production deployments, full SQL feature set
- **MySQL**: Enterprise on-premise (often existing infrastructure)

**Database Strategy**:

- database/sql with driver-specific implementations for database access
- Lowest-common-denominator SQL works across all three databases
- Database-specific migrations (`migrations/sqlite/`, `migrations/postgres/`, `migrations/mysql/`)
- Connection pooling: 16 connections per service instance

**Shared Access**: Both `tk-sensor-api` and `tk-web-ui` share same database with SQLite multiple writer support or PostgreSQL/MySQL native concurrency.

**Cross-References**:

- Database Backend: Complete backend specification with schema design
- Database Migrations: Migration organization per database

**Example**:

```bash
# SQLite (default)
./trapperkeeper sensor-api --data-dir /var/lib/trapperkeeper
# → Uses /var/lib/trapperkeeper/trapperkeeper.db

# PostgreSQL (production)
./trapperkeeper sensor-api --db-url postgres://localhost/trapperkeeper

# MySQL (enterprise)
./trapperkeeper sensor-api --db-url mysql://localhost/trapperkeeper
```

### Explicit Migration Strategy

Migrations require explicit operator approval preventing accidental schema changes.

**Migration Approach**:

- Separate `migrate` subcommand for running migrations
- Services refuse to start if migrations pending
- Clear error message with migration command
- Migration tracking table records applied migrations with checksums
- Checksum validation prevents modified migrations

**Key Principles**:

- Operator control: Full visibility into schema changes before applying
- Safety: Application won't start with stale schema
- Audit trail: Migration tracking table provides complete history
- Integrity: Checksum validation detects modified migrations

**Cross-References**:

- Database Migrations: Complete migration strategy and tracking
- Database Backend: Multi-database migration organization

**Example**:

```bash
# Attempt to start service with pending migrations
./trapperkeeper sensor-api
# → Error: Database schema out of date
#    Run: ./trapperkeeper migrate --db-url <url>

# Run migrations explicitly
./trapperkeeper migrate --db-url postgres://localhost/trapperkeeper
# → Applied 3 migrations (001_initial_schema, 002_add_indices, 003_add_columns)

# Now service starts successfully
./trapperkeeper sensor-api
# → Started sensor-api on port 50051
```

### Unified CLI with Subcommands

Single binary with cobra-based subcommands for all services and tools.

**CLI Structure**:

```
trapperkeeper [GLOBAL_FLAGS] <SUBCOMMAND> [FLAGS]

Subcommands:
  sensor-api     Start sensor API service (gRPC)
  web-ui         Start web UI service (HTTP)
  migrate        Run database migrations

Global Flags:
  --data-dir <PATH>    Data storage directory
  --db-url <URL>       Database connection string
  --log-level <LEVEL>  Logging level (trace|debug|info|warn|error)
```

**Benefits**:

- Single binary simplifies distribution
- Consistent flag parsing across subcommands
- Excellent help text generation from cobra
- Runtime validation of CLI structure

**Cross-References**:

- CLI Design: Complete cobra configuration and subcommand patterns
- Binary Distribution: Single binary architecture

### net/http Web Framework

Standard library net/http for HTTP service providing server-side rendered HTML with CSRF protection.

**Framework Selection**:

- Standard library middleware patterns for composable request handling
- html/template for server-side rendering (no JavaScript required)
- Database-backed session storage using custom middleware
- CSRF middleware with double-submit cookie pattern

**Web UI Characteristics**:

- Pure server-side rendering (no JavaScript in MVP)
- Form-based interactions with CSRF protection
- Session management with sliding expiration
- Static asset serving with embedded assets

**Cross-References**:

- Web Framework: Complete net/http configuration, CSRF, form validation, static assets
- Authentication and User Management: Cookie-based authentication using net/http

### Health Check Endpoints

Standardized endpoints for container orchestration and monitoring.

**Standard Endpoints**:

- `/healthz`: Liveness check (returns 200 if process running)
- `/readyz`: Readiness check (returns 200 if database pingable, 503 if unavailable)
- `/api/v1/stats/prometheus`: Metrics endpoint (sensor-api only)

**Orchestration Integration**:

- Kubernetes liveness probes use `/healthz`
- Kubernetes readiness probes use `/readyz`
- Prometheus scrapes `/api/v1/stats/prometheus`
- Health checks respond within 1 second timeout

**Cross-References**:

- Health Endpoints: Complete endpoint specifications and metrics

## Consequences

**Benefits:**

- Zero-configuration startup: SQLite default enables instant evaluation
- Production-ready: PostgreSQL/MySQL support for scale
- Operational safety: Explicit migrations prevent accidental schema changes
- Configuration flexibility: Multiple sources adapt to deployment context
- Deployment simplicity: Single binary with subcommands
- Container-friendly: Standard health checks integrate with orchestration
- Framework stability: net/http standard library provides battle-tested middleware patterns

**Trade-offs:**

- Extra deployment step: Migrations require manual execution
- Configuration complexity: Three-tier precedence requires understanding
- SQLite limitations: Concurrent write limitations for high-throughput (mitigated by multiple writer support)
- Framework learning curve: Tower middleware requires understanding Service/Layer traits

**Operational Implications:**

- Database migrations must be run before service deployment
- Configuration precedence must be documented for operators
- Health check endpoints must be configured in container orchestration
- Connection pool sizing affects concurrent request capacity
- SQLite suitable for development/evaluation, PostgreSQL/MySQL for production

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- Configuration Management: Maps to multi-source configuration section, provides complete precedence rules
- Database Backend: Maps to database section, provides SQLite/PostgreSQL/MySQL support
- Database Migrations: Maps to explicit migrations section, provides migration tracking
- CLI Design: Maps to unified CLI section, provides cobra configuration
- Web Framework: Maps to net/http section, provides framework selection rationale
- Health Endpoints: Maps to health checks section, provides endpoint specifications

**Dependencies** (foundational documents):

- Principles Architecture: Establishes MVP simplicity principle informing operational choices
- Architecture Overview: Two-service model requiring operational coordination

**References** (related hubs/documents):

- Security: Configuration Security: Secrets enforcement in configuration
- Security: Authentication and User Management: Session management using scs (alexedwards/scs)
- Security: API Authentication: HMAC secret loading from environment

**Extended by**:

- TLS/HTTPS Strategy: net/http middleware for TLS configuration
- Binary Distribution: Subcommand packaging in single binary
