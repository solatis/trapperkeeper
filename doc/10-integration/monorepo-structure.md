---
doc_type: spoke
status: active
primary_category: architecture
hub_document: doc/10-integration/README.md
tags:
  - monorepo
  - go-module
  - directory-structure
  - version-synchronization
---

# Monorepo Structure

## Context

TrapperKeeper requires directory structure supporting Go module conventions, polyglot SDK bindings (Python, Java, Go native SDKs), runtime database migrations, and version synchronization across all components. This document specifies the Go-centric monorepo layout with single go.mod at repository root and internal/ package organization.

**Hub Document**: This spoke is part of [Integration Overview](README.md). See the hub's Monorepo Structure section for strategic context.

## Go Module at Repository Root

### Module Definition

**Root Configuration** (go.mod):

```go
module github.com/trapperkeeper/trapperkeeper

go 1.23  // Always use latest stable Go release

require (
    github.com/google/uuid v1.6.0
    github.com/jmoiron/sqlx v1.4.0
    github.com/spf13/cobra v1.8.0
    github.com/spf13/viper v1.18.2
    google.golang.org/grpc v1.62.1
    google.golang.org/protobuf v1.33.0
)

// Versions pinned at latest stable on first use.
// Minor/patch upgrades: apply freely.
// Major upgrades: require expert decision.
// See: dependency-management.md
```

**Benefits**:

- Single module for entire Go codebase
- Unified dependency management
- Single version source of truth
- Simplified build with `go build ./...`
- internal/ package visibility enforcement

### Directory Structure

**Repository Root Layout**:

```
trapperkeeper/
├── go.mod                        # Go module definition
├── go.sum                        # Dependency checksums
├── vendor/                       # Vendored Go dependencies (committed)
├── cmd/
│   └── trapperkeeper/            # Main binary
│       └── main.go
├── internal/
│   ├── types/                    # Domain models
│   ├── rules/                    # Rule logic
│   ├── core/                     # Server code
│   ├── db/                       # Database access
│   └── api/                      # gRPC server
├── proto/
│   └── trapperkeeper/
│       └── sensor/
│           └── v1/               # Protocol buffer definitions
├── migrations/
│   ├── sqlite/                   # SQLite migrations
│   ├── postgres/                 # PostgreSQL migrations
│   └── mysql/                    # MySQL migrations
├── sdks/
│   ├── python/                   # Python SDK
│   ├── java/                     # Java SDK
│   └── go/                       # Go SDK (separate module)
└── tests/
    ├── integration/              # Integration tests
    └── fixtures/                 # Test fixtures
```

**Rationale**: Single module with internal/ packages provides clean visibility control. Separate SDK modules for language-specific distributions.

**Cross-Reference**: See [Integration Overview](README.md) for complete module architecture description and dependency graph.

## Internal Package Architecture

### 1. internal/types (Domain Models)

**Purpose**: Ultra-thin package with shared types used across client and server.

**Directory Structure**:

```
internal/types/
├── rule.go            # Rule struct
├── field_path.go      # FieldPath type
├── event.go           # Event struct
├── condition.go       # Condition types
└── error.go           # Error types
```

**Import Path**: `github.com/trapperkeeper/trapperkeeper/internal/types`

**Dependencies**: Zero external dependencies beyond encoding/json

**Example**:

```go
// internal/types/rule.go
package types

import "github.com/google/uuid"

type Rule struct {
    RuleID      uuid.UUID  `json:"rule_id"`
    TenantID    uuid.UUID  `json:"tenant_id"`
    Name        string     `json:"name"`
    Description *string    `json:"description,omitempty"`
    Enabled     bool       `json:"enabled"`
    OrGroups    []OrGroup  `json:"or_groups"`
    Action      RuleAction `json:"action"`
}
```

**Key Constraint**: Zero external dependencies beyond encoding/json (prevents dependency bloat in SDKs).

### 2. proto/ (Protocol Buffer Definitions)

**Purpose**: Own gRPC protocol buffer definitions and generated Go code.

**Directory Structure**:

```
proto/
└── trapperkeeper/
    └── sensor/
        └── v1/
            ├── sensor_api.proto
            ├── rule.proto
            ├── field_path.proto
            └── event.proto
```

**Generated Code Location**: `proto/trapperkeeper/sensor/v1/` (Go files generated alongside .proto files)

**Generation**:

```bash
# Using buf
buf generate

# Or using protoc
protoc --go_out=. --go-grpc_out=. proto/trapperkeeper/sensor/v1/*.proto
```

**Consumers**: Native SDKs (gRPC client), internal/api (gRPC server)

### 3. internal/rules (Rule Compilation and Validation)

**Purpose**: Centralized rule compilation, execution, and runtime validation logic.

**Directory Structure**:

```
internal/rules/
├── parser/            # Rule parser (DNF expressions)
│   ├── parser.go
│   └── dnf.go
├── executor/          # Rule execution
│   ├── executor.go
│   └── evaluator.go
└── validation/        # Runtime validation
    ├── validation.go
    ├── field_path.go
    └── type_coercion.go
```

**Import Path**: `github.com/trapperkeeper/trapperkeeper/internal/rules`

**Dependencies**: internal/types (domain models only)

**Consumers**: Native SDKs (SDK-side evaluation), internal/core (server-side validation), cmd/web-ui (validation)

**Cross-Reference**: See [Data: Rule Expression Language](../04-rule-engine/README.md) for complete rule parsing specification.

### 4. internal/core (Server-Side Shared Code)

**Purpose**: Shared code for web-ui, sensor-api, migrate subcommands.

**Directory Structure**:

```
internal/core/
├── db/                # Database access (jmoiron/sqlx)
│   ├── db.go
│   ├── rules.go
│   └── users.go
├── auth/              # Authentication primitives
│   ├── auth.go
│   ├── hmac.go
│   └── bcrypt.go
└── config/            # Configuration management
    ├── config.go
    └── viper.go
```

**Import Path**: `github.com/trapperkeeper/trapperkeeper/internal/core`

**Dependencies**: internal/types, proto/ (generated code), internal/rules, jmoiron/sqlx, viper

**Example**:

```go
// internal/core/db/rules.go
package db

import (
    "context"
    "github.com/trapperkeeper/trapperkeeper/internal/types"
    "github.com/google/uuid"
    "github.com/jmoiron/sqlx"
)

func FetchRules(ctx context.Context, db *sqlx.DB, tenantID uuid.UUID) ([]types.Rule, error) {
    var rules []types.Rule
    err := db.SelectContext(ctx, &rules,
        "SELECT * FROM rules WHERE tenant_id = ? AND deleted_at IS NULL",
        tenantID)
    return rules, err
}
```

**Critical Constraint**: Does NOT depend on SDK code (avoids server->client dependency).

### 5. cmd/trapperkeeper (Main Binary)

**Purpose**: CLI entry point with subcommands for sensor-api, web-ui, migrate.

**Directory Structure**:

```
cmd/trapperkeeper/
├── main.go            # CLI entry point
└── cmd/
    ├── root.go        # Root command
    ├── sensor_api.go  # gRPC service
    ├── web_ui.go      # HTTP service
    └── migrate.go     # Migration command
```

**Import Path**: `github.com/trapperkeeper/trapperkeeper/cmd/trapperkeeper`

**Dependencies**: internal/core, cobra

**Build Command**:

```bash
go build -o trapperkeeper ./cmd/trapperkeeper
```

**Cross-Reference**: See [Operations: CLI Design](../09-operations/cli-design.md) for complete CLI structure.

## SDK Organization Under sdks/

### Python SDK Structure

**Directory Layout**:

```
sdks/python/
├── pyproject.toml         # Python package configuration
├── README.md
├── trapperkeeper/         # Pure Python code
│   ├── __init__.py
│   ├── client.py          # gRPC client wrapper
│   └── pandas.py          # Pandas integration
└── proto/                 # Generated protobuf code
    └── trapperkeeper/
        └── sensor/
            └── v1/
```

**pyproject.toml** (Python package configuration):

```toml
[build-system]
requires = ["setuptools>=65.0", "wheel"]
build-backend = "setuptools.build_meta"

[project]
name = "trapperkeeper"
version = "0.1.0"
requires-python = ">=3.8"
dependencies = [
    "grpcio>=1.62.0",
    "protobuf>=4.25.0",
    "pandas>=1.5",
]
```

**Installation**:

```bash
cd sdks/python

# Install for development
pip install -e .

# Build wheel
python -m build

# Run tests
pytest tests/
```

### Java SDK Structure

**Directory Layout**:

```
sdks/java/
├── trapperkeeper/         # Java library
│   ├── build.gradle
│   └── src/main/java/
│       └── ai/trapperkeeper/
├── trapperkeeper-spark/   # Spark wrapper
│   ├── build.gradle
│   └── src/main/java/
│       └── ai/trapperkeeper/spark/
└── proto/                 # Generated protobuf code
    └── trapperkeeper/
        └── sensor/
            └── v1/
```

**build.gradle**:

```groovy
plugins {
    id 'java-library'
    id 'maven-publish'
}

group = 'ai.trapperkeeper'
version = '0.1.0'

dependencies {
    implementation 'io.grpc:grpc-netty:1.62.1'
    implementation 'io.grpc:grpc-protobuf:1.62.1'
    implementation 'io.grpc:grpc-stub:1.62.1'
    implementation 'com.google.protobuf:protobuf-java:3.24.0'
}
```

**Build Commands**:

```bash
cd sdks/java

# Build all Java artifacts
./gradlew build

# Run tests
./gradlew test

# Build Spark wrapper
./gradlew :trapperkeeper-spark:build
```

### Go SDK Structure

**Directory Layout**:

```
sdks/go/
├── client.go              # gRPC client wrapper
├── buffer.go              # Event buffering
├── evaluator.go           # Rule evaluation (uses internal/rules)
└── doc.go                 # Package documentation
```

**Key Design Decision**: The Go SDK is a **package within the main module**, not a
separate module. This allows it to import internal/types and internal/rules while
remaining externally importable.

**Rationale**:

- Go's internal/ visibility only blocks imports from _outside_ the module
- Packages in sdks/go/ are inside the main module, so they CAN import internal/
- External users import via the full path, which works for any non-internal package
- No separate go.mod means no version synchronization complexity

**Why not a separate module?** A separate go.mod in sdks/go/ would create a distinct
Go module that CANNOT import internal/ packages from the parent. This would force
either code duplication or moving types/rules out of internal/ (exposing them to all
external consumers). Keeping sdks/go/ as a package avoids this entirely.

**External Import Path**:

```go
import "github.com/trapperkeeper/trapperkeeper/sdks/go"
```

**Build Commands**:

```bash
# Build SDK (from repository root)
go build ./sdks/go/...

# Test SDK
go test ./sdks/go/...

# External users install via:
go get github.com/trapperkeeper/trapperkeeper/sdks/go
```

## Centralized Migrations and Tests

### Migrations Directory

**Location**: Repository root for runtime access.

**Structure**:

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

**Embedding** (using embed.FS):

```go
// internal/core/db/migrations.go
package db

import _ "embed"

//go:embed migrations/sqlite/*.sql
var sqliteMigrations embed.FS
```

**Rationale**: Centralized location accessible to both internal/core (library) and cmd/trapperkeeper (binary).

**Cross-Reference**: See [Operations: Database Migrations](../09-operations/database-migrations.md) for complete migration strategy.

### Tests Directory

**Location**: Repository root for integration tests.

**Structure**:

```
tests/
├── integration/
│   ├── go_tests/
│   ├── python_tests/
│   ├── java_tests/
│   └── docker/
│       └── docker-compose.yml  # Ephemeral test environment
└── fixtures/                   # Test fixtures
    ├── sample_rules.json
    └── sample_events.jsonl
```

**Integration Tests**:

```bash
# Run all integration tests
./scripts/dev/test-all.sh

# Docker-based integration tests
cd tests/integration/docker
docker-compose up -d
go test ./tests/integration/go_tests
docker-compose down
```

**Cross-Reference**: See [Principles: Testing Philosophy](../01-principles/testing-philosophy.md) for complete testing approach.

## Version Synchronization Strategy

### Go Module Version

**go.mod** (single source of truth):

```go
module github.com/trapperkeeper/trapperkeeper

go 1.23  // Always latest stable; see dependency-management.md
```

**Versioning**: Go modules use semantic versioning via git tags (v0.1.0, v1.0.0, etc.)

### Version Propagation Script

**Script** (scripts/release/sync-versions.sh):

```bash
#!/bin/bash
# Synchronize version from git tag to Python and Java SDKs

VERSION=$(git describe --tags --abbrev=0 | sed 's/^v//')

# Update Python SDK
sed -i '' "s/^version = .*/version = \"$VERSION\"/" sdks/python/pyproject.toml

# Update Java SDK
sed -i '' "s/^version = .*/version = '$VERSION'/" sdks/java/trapperkeeper/build.gradle
sed -i '' "s/^version = .*/version = '$VERSION'/" sdks/java/trapperkeeper-spark/build.gradle

echo "Synchronized version $VERSION across all SDKs"
```

**Usage**:

```bash
# After tagging release
git tag v0.1.0
./scripts/release/sync-versions.sh
```

### Release Process Integration

**Step-by-Step**:

1. Tag release: `git tag v0.1.0`
2. Run `scripts/release/sync-versions.sh` to propagate version
3. Commit version changes: `git add . && git commit -m "Bump version to 0.1.0"`
4. Push: `git push && git push --tags`
5. CI builds all artifacts from tag
6. Publish: Go module (via proxy), Python wheel to PyPI, Java JAR to Maven Central

## Build Commands Reference

### Go Core and Binary

```bash
# Build entire module
go build ./...

# Build specific package
go build ./internal/core

# Test entire module
go test ./...

# Test specific package
go test ./internal/rules

# Run binary
go run ./cmd/trapperkeeper sensor-api --port 50051

# Build release binary
go build -ldflags="-s -w" -o trapperkeeper ./cmd/trapperkeeper

# Cross-compile for Linux
GOOS=linux GOARCH=amd64 go build -o trapperkeeper-linux ./cmd/trapperkeeper
```

### Python SDK

```bash
cd sdks/python

# Build wheel (production)
python -m build

# Install for development (editable install)
pip install -e .

# Run Python tests
pytest tests/ -v

# Type checking
mypy trapperkeeper/

# Linting
black trapperkeeper/ tests/
ruff check trapperkeeper/ tests/
```

### Java SDK

```bash
cd sdks/java

# Build all Java artifacts
./gradlew build

# Build specific module
./gradlew :trapperkeeper:build
./gradlew :trapperkeeper-spark:build

# Run tests
./gradlew test

# Run tests for specific module
./gradlew :trapperkeeper:test

# Generate JAR
./gradlew jar

# Publish to Maven Local
./gradlew publishToMavenLocal
```

### Go SDK

```bash
cd sdks/go

# Build SDK
go build ./...

# Run tests
go test ./...

# Run tests with coverage
go test -cover ./...
```

### Cross-Language Build (All Components)

```bash
# Script: scripts/dev/build-all.sh
#!/bin/bash
set -e

# Go core
go build ./...

# Python SDK
cd sdks/python
python -m build
cd ../..

# Java SDK
cd sdks/java
./gradlew build
cd ../..

# Go SDK
cd sdks/go
go build ./...
cd ../..

echo "All components built successfully"
```

## Related Documents

**Dependencies** (read these first):

- [Integration Overview](README.md): Strategic context for monorepo structure
- [Principles: Architectural Principles](../01-principles/README.md): Simplicity principle informing Go module layout

**Related Spokes** (siblings in this hub):

- [Package Separation](package-separation.md): Module architecture rationale and SDK implications

**Implements** (realizes these decisions):

- [Architecture: SDK Model](../02-architecture/sdk-model.md): SDK directory structure for Python, Java, and Go implementations
- [Architecture: Service Architecture](../02-architecture/README.md): Implementation structure for internal/api and cmd/web-ui services
- [Operations: Database Migrations](../09-operations/database-migrations.md): Migrations location at root for runtime access
- [Performance: Batch Processing and Vectorization](../05-performance/batch-processing.md): Pandas and Spark wrapper locations in SDK directories

**Extended by**:

- [Architecture: Binary Distribution](../02-architecture/binary-distribution.md): Single binary distribution from cmd/trapperkeeper
