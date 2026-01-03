---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: doc/02-architecture/README.md
tags:
  - single-binary
  - subcommands
  - go-build
  - distribution
---

# Binary Distribution Strategy

## Context

TrapperKeeper requires unified distribution packaging both services (`tk-sensor-api` and `tk-web-ui`) and operational tooling (database migrations) in a single artifact. This simplifies deployment, ensures version consistency across components, and reduces operational complexity for five-engineer team.

Traditional multi-binary approaches require coordinating versions across artifacts, managing separate deployment pipelines, and handling dependency conflicts between services. Single binary with subcommands eliminates these coordination problems while maintaining service independence at runtime.

**Hub Document**: This document is part of the Architecture Hub. See [Architecture Overview](README.md) for strategic context on unified binary distribution within TrapperKeeper's two-service architecture.

## Single Binary Architecture

All services and operational tooling packaged as subcommands in single `trapperkeeper` binary.

### Binary Structure

```
trapperkeeper                    # Main binary (~15-20MB)
├── sensor-api                   # Subcommand: gRPC service
├── web-ui                       # Subcommand: HTTP service
└── migrate                      # Subcommand: Database migrations
```

**Usage Examples**:

```bash
# Run sensor API
./trapperkeeper sensor-api --port 50051 --data-dir /var/lib/trapperkeeper

# Run web UI
./trapperkeeper web-ui --port 8080 --data-dir /var/lib/trapperkeeper

# Run database migrations
./trapperkeeper migrate --db-url postgres://localhost/trapperkeeper
```

**Key Characteristics**:

- Single executable file (~15-20MB compressed)
- All subcommands share dependencies (Go stdlib, goroutines, database/sql)
- Version number applies to entire binary
- Dead code elimination reduces binary size (enabled by default in Go)
- Embedded database migrations (no external files)

**Cross-References**:

- Architecture Overview Section 3: Unified binary distribution strategy
- CLI Configuration: Subcommand argument parsing with cobra
- Database Migrations: Embedded migration strategy using embed.FS

## Go Module Configuration

Go modules enable unified build with shared dependencies.

### Module Definition

`go.mod` at repository root:

```go
module github.com/trapperkeeper/trapperkeeper

go 1.23  // Always latest stable; see 10-integration/dependency-management.md

require (
    google.golang.org/grpc v1.60.0
    google.golang.org/protobuf v1.32.0
    github.com/spf13/cobra v1.8.0
    github.com/lib/pq v1.10.9          // PostgreSQL driver
    github.com/go-sql-driver/mysql v1.7.1  // MySQL driver
    modernc.org/sqlite v1.28.0        // SQLite driver
    // ... other shared dependencies
)

// Version policy: latest stable on first use, minor upgrades freely,
// major upgrades require expert decision.
```

**Directory Structure**:

```
github.com/trapperkeeper/trapperkeeper/
├── go.mod                    # Module definition
├── go.sum                    # Dependency checksums
├── cmd/
│   └── trapperkeeper/
│       └── main.go           # Main binary entrypoint
├── internal/
│   ├── types/                # Internal types package
│   ├── protobuf/             # Protocol buffer definitions
│   ├── rules/                # Rule engine
│   ├── client/               # Client SDK
│   └── core/                 # Core service logic
└── sdks/
    ├── python/
    └── java/
```

**Benefits**:

- Shared dependency resolution: Single `go.sum` for entire module
- Unified version management: Version embedded via `-ldflags` at build time
- Consistent build flags: All packages use same Go toolchain settings
- Faster builds: Go caches dependencies across all packages

**Cross-References**:

- Monorepo Directory Structure Section 2: Five-package architecture
- Client/Server Package Separation: Package organization and dependencies

### Binary Package Structure

`cmd/trapperkeeper/main.go`:

```go
package main

import (
    "fmt"
    "os"

    "github.com/spf13/cobra"
    "github.com/trapperkeeper/trapperkeeper/internal/core"
)

var (
    // Version information injected at build time via -ldflags
    version   = "dev"
    commit    = "unknown"
    buildDate = "unknown"
)

var (
    // Global flags (apply to all subcommands)
    dataDir string
    dbURL   string
)

var rootCmd = &cobra.Command{
    Use:   "trapperkeeper",
    Short: "TrapperKeeper: Schema-agnostic data pipeline observability",
    Long:  `TrapperKeeper provides observability for data pipelines with schema-agnostic rule evaluation.`,
}

var sensorAPICmd = &cobra.Command{
    Use:   "sensor-api",
    Short: "Start sensor API service (gRPC)",
    RunE: func(cmd *cobra.Command, args []string) error {
        port, _ := cmd.Flags().GetInt("port")
        hmacSecret, _ := cmd.Flags().GetString("hmac-secret")
        return core.RunSensorAPI(port, hmacSecret, dataDir)
    },
}

var webUICmd = &cobra.Command{
    Use:   "web-ui",
    Short: "Start web UI service (HTTP)",
    RunE: func(cmd *cobra.Command, args []string) error {
        port, _ := cmd.Flags().GetInt("port")
        return core.RunWebUI(port, dataDir)
    },
}

var migrateCmd = &cobra.Command{
    Use:   "migrate",
    Short: "Run database migrations",
    RunE: func(cmd *cobra.Command, args []string) error {
        dryRun, _ := cmd.Flags().GetBool("dry-run")
        return core.RunMigrations(dryRun, dbURL)
    },
}

func init() {
    // Global flags
    rootCmd.PersistentFlags().StringVar(&dataDir, "data-dir", "", "Data directory path")
    rootCmd.PersistentFlags().StringVar(&dbURL, "db-url", "", "Database connection URL")

    // sensor-api flags
    sensorAPICmd.Flags().Int("port", 50051, "gRPC server port")
    sensorAPICmd.Flags().String("hmac-secret", "", "HMAC secret for authentication")
    sensorAPICmd.MarkFlagRequired("hmac-secret")

    // web-ui flags
    webUICmd.Flags().Int("port", 8080, "HTTP server port")

    // migrate flags
    migrateCmd.Flags().Bool("dry-run", false, "Show migrations without applying")

    rootCmd.AddCommand(sensorAPICmd, webUICmd, migrateCmd)
}

func main() {
    if err := rootCmd.Execute(); err != nil {
        fmt.Fprintln(os.Stderr, err)
        os.Exit(1)
    }
}
```

**Cross-References**:

- CLI Configuration: Complete cobra configuration patterns
- Service Architecture: Implementation of sensor-api and web-ui services

## Build Optimization

Static binary builds and build flags reduce binary size and improve portability.

### Static Binary Configuration

Build command for optimized release binary:

```bash
# Static binary for Linux (no CGO dependencies)
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build \
  -ldflags="-s -w -X main.version=0.1.0 -X main.commit=$(git rev-parse HEAD) -X main.buildDate=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  -trimpath \
  -o trapperkeeper \
  ./cmd/trapperkeeper

# Cross-compile for macOS ARM64
CGO_ENABLED=0 GOOS=darwin GOARCH=arm64 go build \
  -ldflags="-s -w -X main.version=0.1.0" \
  -trimpath \
  -o trapperkeeper-darwin-arm64 \
  ./cmd/trapperkeeper

# Cross-compile for Windows
CGO_ENABLED=0 GOOS=windows GOARCH=amd64 go build \
  -ldflags="-s -w -X main.version=0.1.0" \
  -trimpath \
  -o trapperkeeper.exe \
  ./cmd/trapperkeeper
```

**Build Flag Explanations**:

- `CGO_ENABLED=0`: Disables CGO for fully static binary (no libc dependencies)
- `-ldflags="-s -w"`: Strips debug info (-s) and DWARF symbol table (-w)
- `-ldflags="-X"`: Injects version information at build time
- `-trimpath`: Removes filesystem paths from binary for reproducible builds
- Dead code elimination: Enabled by default in Go (no configuration needed)

**Optimization Benefits**:

- Static binaries: No runtime dependencies beyond kernel syscalls
- Cross-compilation: Build for any platform without toolchain setup
- Version injection: Embedded version accessible via `trapperkeeper --version`
- Smaller binaries: Strip flags reduce size by 20-30%

**Build Time Tradeoffs**:

- Debug builds: ~30-60 seconds (no optimization flags)
- Release builds: ~1-2 minutes (with -ldflags stripping)
- CI caching: Go module cache speeds up subsequent builds

**Cross-References**:

- Testing Integration Patterns Section 5: CI/CD build configuration

## Embedded Migrations

Database migrations embedded at compile time using embed.FS.

### Migration Embedding

`internal/core/db/migrations.go`:

```go
package db

import (
    "database/sql"
    "embed"
    "fmt"

    "github.com/golang-migrate/migrate/v4"
    "github.com/golang-migrate/migrate/v4/database/postgres"
    "github.com/golang-migrate/migrate/v4/source/iofs"
)

//go:embed migrations/*.sql
var migrationFiles embed.FS

// RunMigrations applies embedded SQL migrations to the database
func RunMigrations(db *sql.DB, databaseName string) error {
    // Create migration source from embedded filesystem
    source, err := iofs.New(migrationFiles, "migrations")
    if err != nil {
        return fmt.Errorf("failed to create migration source: %w", err)
    }

    // Create database driver
    driver, err := postgres.WithInstance(db, &postgres.Config{})
    if err != nil {
        return fmt.Errorf("failed to create database driver: %w", err)
    }

    // Create migration instance
    m, err := migrate.NewWithInstance("iofs", source, databaseName, driver)
    if err != nil {
        return fmt.Errorf("failed to create migrator: %w", err)
    }

    // Apply migrations
    if err := m.Up(); err != nil && err != migrate.ErrNoChange {
        return fmt.Errorf("migration failed: %w", err)
    }

    return nil
}
```

**Directory Structure**:

```
internal/core/db/
├── migrations.go                  # Migration embedding logic
└── migrations/
    ├── sqlite/
    │   ├── 000001_initial_schema.up.sql
    │   ├── 000001_initial_schema.down.sql
    │   ├── 000002_add_indices.up.sql
    │   └── 000002_add_indices.down.sql
    ├── postgres/
    │   ├── 000001_initial_schema.up.sql
    │   ├── 000001_initial_schema.down.sql
    │   ├── 000002_add_indices.up.sql
    │   └── 000002_add_indices.down.sql
    └── mysql/
        ├── 000001_initial_schema.up.sql
        ├── 000001_initial_schema.down.sql
        ├── 000002_add_indices.up.sql
        └── 000002_add_indices.down.sql
```

**Benefits**:

- No external migration files required at runtime
- Migration integrity guaranteed (embedded in binary)
- Version synchronization automatic (binary version matches migrations)
- Simplifies deployment (single binary contains everything)

**Cross-References**:

- Database Migrations: Complete migration strategy
- Database Backend: Multi-database support

## Distribution Channels

Binary distributed through multiple channels for different use cases.

### GitHub Releases

Primary distribution channel for manual downloads:

```bash
# Linux x86_64
https://github.com/trapperkeeper/trapperkeeper/releases/download/v0.1.0/trapperkeeper-v0.1.0-linux-x86_64

# macOS ARM64
https://github.com/trapperkeeper/trapperkeeper/releases/download/v0.1.0/trapperkeeper-v0.1.0-darwin-arm64

# Windows x86_64
https://github.com/trapperkeeper/trapperkeeper/releases/download/v0.1.0/trapperkeeper-v0.1.0-windows-x86_64.exe
```

**Release Process**:

1. Tag release: `git tag v0.1.0`
2. CI builds binaries for all platforms
3. GitHub Actions creates release with artifacts
4. Changelog auto-generated from commit messages

### Docker Images

Containerized distribution for orchestration platforms:

```bash
# Pull image
docker pull trapperkeeper/trapperkeeper:v0.1.0

# Run sensor API
docker run -p 50051:50051 trapperkeeper/trapperkeeper:v0.1.0 sensor-api

# Run web UI
docker run -p 8080:8080 trapperkeeper/trapperkeeper:v0.1.0 web-ui
```

**Docker Image Structure**:

```dockerfile
# Build stage
FROM golang:1.25-alpine as builder
WORKDIR /build
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build \
    -ldflags="-s -w -X main.version=${VERSION}" \
    -trimpath \
    -o trapperkeeper \
    ./cmd/trapperkeeper

# Runtime stage
FROM alpine:latest
RUN apk --no-cache add ca-certificates
COPY --from=builder /build/trapperkeeper /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/trapperkeeper"]
CMD ["--help"]
```

**Image Tags**:

- `v0.1.0`: Specific version
- `v0.1`: Minor version (follows latest patch)
- `latest`: Latest stable release
- `edge`: Latest commit to main branch

### Package Managers (Future)

Post-MVP distribution through package managers:

- **Homebrew**: `brew install trapperkeeper`
- **APT/YUM**: System package repositories for Linux
- **Go install**: `go install github.com/trapperkeeper/trapperkeeper/cmd/trapperkeeper@latest`

**Cross-References**:

- CLI Configuration: Installation and usage patterns

## Version Management

Single version number applies to entire system.

### Version Synchronization

All components share version from workspace:

- Binary: `trapperkeeper --version` → `trapperkeeper 0.1.0`
- API service: Reports version in health check endpoint
- Web UI: Displays version in footer
- SDKs: Python/Java packages inherit version via build script

**Version Propagation Script**:

```bash
#!/bin/bash
# scripts/release/sync-versions.sh

VERSION=$1

if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    exit 1
fi

# Update Python SDK
sed -i '' "s/^version = .*/version = \"$VERSION\"/" sdks/python/pyproject.toml

# Update Java SDK
sed -i '' "s/^version = .*/version = '$VERSION'/" sdks/java/trapperkeeper/build.gradle

echo "Synced version $VERSION to SDKs"
```

**Release Workflow**:

1. Run `scripts/release/sync-versions.sh 0.1.0` to update SDK versions
2. Commit version changes
3. Tag release: `git tag v0.1.0`
4. Push tag: `git push --tags`
5. CI builds binaries with version injected via `-ldflags`
6. CI publishes all artifacts (binaries, Docker images, SDK packages)

**Cross-References**:

- Monorepo Directory Structure Appendix B: Complete version synchronization strategy

## Deployment Patterns

Single binary supports multiple deployment scenarios.

### Container Orchestration (Kubernetes)

```yaml
# k8s/sensor-api.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: trapperkeeper-sensor-api
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: sensor-api
          image: trapperkeeper/trapperkeeper:v0.1.0
          args: ["sensor-api", "--port", "50051"]
          env:
            - name: TK_HMAC_SECRET
              valueFrom:
                secretKeyRef:
                  name: trapperkeeper-secrets
                  key: hmac-secret
          ports:
            - containerPort: 50051
```

### Systemd Services (Bare Metal)

```ini
# /etc/systemd/system/trapperkeeper-sensor-api.service
[Unit]
Description=TrapperKeeper Sensor API
After=network.target

[Service]
Type=simple
User=trapperkeeper
ExecStart=/usr/local/bin/trapperkeeper sensor-api \
  --port 50051 \
  --data-dir /var/lib/trapperkeeper
Environment=TK_HMAC_SECRET=...
Restart=always

[Install]
WantedBy=multi-user.target
```

### Docker Compose (Development)

```yaml
# docker-compose.yml
version: "3.8"
services:
  sensor-api:
    image: trapperkeeper/trapperkeeper:latest
    command: sensor-api --port 50051
    ports:
      - "50051:50051"
    environment:
      - TK_HMAC_SECRET=${TK_HMAC_SECRET}

  web-ui:
    image: trapperkeeper/trapperkeeper:latest
    command: web-ui --port 8080
    ports:
      - "8080:8080"

  db:
    image: postgres:16
    environment:
      - POSTGRES_DB=trapperkeeper
```

**Cross-References**:

- Service Architecture: Two-service deployment model
- Configuration Management: Environment variable configuration

## Related Documents

**Dependencies** (read these first):

- Architecture Overview: Unified binary distribution strategy
- Service Architecture: Two-service model requiring unified distribution

**Related Spokes** (siblings in this hub):

- API Service Architecture: sensor-api subcommand implementation
- SDK Model: Go SDK (sdks/go/) imports internal/rules but avoids internal/core

**Extended by**:

- CLI Configuration: Subcommand argument parsing with cobra
- Database Migrations: Embedded migration strategy
- Monorepo Directory Structure: Module organization enabling single binary
- Client/Server Package Separation: Five-package architecture
