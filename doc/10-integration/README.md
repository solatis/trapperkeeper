---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: architecture
consolidated_spokes:
  - package-separation.md
  - monorepo-structure.md
  - dependency-management.md
tags:
  - integration
  - monorepo
  - module-architecture
  - code-organization
---

# Integration Overview

## Context

TrapperKeeper requires code organization supporting polyglot SDK development (Python, Java, Go), lean client distributions without server dependencies, and version synchronization across all components. Traditional approaches create problems: separate repositories complicate version coordination, monolithic packages force SDKs to bundle unnecessary server code, and complex directory structures increase cognitive load for small teams.

This hub consolidates integration decisions establishing monorepo structure, Go module architecture with clean separation between client and server concerns, and SDK organization patterns enabling independent builds while maintaining version consistency.

## Decision

We implement **Go-centric monorepo with modular package architecture**: go.mod at repository root, internal/ packages for shared code (types, protobuf, rules, client, core), cmd/ directory for binaries, sdks/ directory for polyglot bindings, and centralized migrations/ and tests/ directories.

This document serves as the integration hub providing strategic overview with cross-references to detailed implementation documents for monorepo structure and package separation.

### Go Module Architecture

**Core Principle**: Minimal package split separating client from server while establishing clear ownership for protocol types and shared validation logic.

**Package Responsibilities**:

1. **internal/types**: Ultra-thin domain models (Rule, FieldPath, Event, Condition, errors)
   - Zero external dependencies (encoding/json only for serialization)
   - Shared across client and server
   - ~10KB binary size impact

2. **internal/protobuf**: Protocol buffer compilation and generated types
   - Owns gRPC protocol definitions
   - Compiled at build time from proto/ directory via protoc-gen-go
   - Shared by client (gRPC client) and server (gRPC server)

3. **internal/rules**: Rule compilation, execution, and validation logic
   - Centralizes rule parsing (DNF expressions)
   - Centralizes validation logic (shared by SDK, server, web UI)
   - Depends only on internal/types (no protocol dependencies)

4. **sdks/go/**: Go SDK (package within main module)
   - gRPC client communication (uses proto/ generated types)
   - Event buffering and batching
   - Rule evaluation (imports internal/rules)
   - Externally importable: `go get .../trapperkeeper/sdks/go`
   - Depends on: internal/types, internal/rules, proto/

5. **internal/core**: Server-side shared code
   - Database access (dotsql + jmoiron/sqlx with query organization)
   - Authentication primitives (HMAC validation, API key management, bcrypt)
   - Configuration management (viper or environment variables)
   - Depends on: internal/types, internal/protobuf, internal/rules

**Key Property**: Go SDK (sdks/go/) imports internal/types and internal/rules but avoids
internal/core (no server dependencies). Python/Java SDKs use gRPC with generated protobuf
types only.

**Cross-References**:

- Monorepo Structure: Complete package descriptions and dependency graph
- Package Separation: Rationale for modular split and SDK implications
- Dependency Management: Go/Python/Java versioning policies and vendoring strategy

### Monorepo Structure

**Go-Centric Layout**: go.mod at repository root with internal/ package organization.

**Top-Level Organization**:

```
trapperkeeper/
├── go.mod                        # Module root
├── internal/
│   ├── types/                    # Domain models
│   ├── protobuf/                 # Protocol buffers
│   ├── rules/                    # Rule logic
│   ├── client/                   # Client SDK
│   └── core/                     # Server code
├── cmd/
│   └── trapperkeeper/            # Main binary
├── sdks/                         # Language SDKs
│   ├── python/
│   └── java/
├── migrations/                   # Database migrations
├── tests/                        # Integration tests
└── proto/                        # Protocol definitions
```

**Benefits**:

- Flat Go layout easy to navigate (5-engineer team)
- Clear separation between Go core and language SDKs
- Centralized migrations and tests accessible to all packages
- Version synchronization via go.mod

**Cross-Reference**: Monorepo Structure for complete directory organization and build commands.

### SDK Organization

**sdks/ Directory**: Language-specific SDK bindings with both language code and Go client package.

**Python SDK** (sdks/python/):

```
sdks/python/
├── trapperkeeper/           # Pure Python code (Pandas wrapper)
├── tk-python/               # CGo binding package
└── pyproject.toml           # Python build configuration
```

**Java SDK** (sdks/java/):

```
sdks/java/
├── trapperkeeper/           # Java library
├── trapperkeeper-spark/     # Spark wrapper
└── tk-java/                 # JNI binding package
```

**Go SDK** (sdks/go/):

```
sdks/go/
├── client.go                # gRPC client wrapper
├── buffer.go                # Event buffering
└── evaluator.go             # Rule evaluation
```

**Go SDK Design**: Unlike Python and Java, the Go SDK is a **package within the main
module** (no separate go.mod). This allows it to import internal/types and
internal/rules while remaining externally importable via
`go get github.com/trapperkeeper/trapperkeeper/sdks/go`.

**Key Constraint**: SDK packages avoid server dependencies (internal/core).

**Build Independence**: Each SDK can be built and tested independently while maintaining version sync via go.mod configuration.

**Cross-Reference**: Monorepo Structure for complete SDK build commands and version synchronization.

### Dependency Graph

**Canonical Dependency Chains**:

**Server Chain**:

```
trapperkeeper binary -> internal/core -> internal/protobuf
                                      -> internal/rules -> internal/types

tk-web-ui -> internal/core -> internal/protobuf
                           -> internal/rules -> internal/types

tk-sensor-api -> internal/core -> internal/protobuf
                                -> internal/rules -> internal/types
```

**SDK Chain**:

```
Go SDK (sdks/go/) -> internal/rules -> internal/types
                  -> proto/ (generated protobuf)

Python/Java SDKs -> gRPC (generated protobuf only)
                 -> own rule evaluation implementation
```

**Key Properties**:

- Go SDK imports internal/rules and internal/types (same module, allowed)
- Go SDK avoids internal/core (no server dependencies)
- Python/Java SDKs use generated protobuf types (cannot import Go packages)
- internal/rules centralizes validation shared by Go SDK, server, web UI
- internal/types remains zero-dependency (preserves simplicity)
- Protocol changes (proto/) don't trigger rule logic recompilation (internal/rules)

**Cross-Reference**: Package Separation for complete dependency graph and rationale.

## Consequences

**Benefits:**

- **Lean SDK Distributions**: 80-90% binary size reduction (5-10MB vs 50+MB)
- **Clear Ownership**: Protocol types (internal/protobuf), validation logic (internal/rules), domain models (internal/types) have single owners
- **Build Efficiency**: Go module builds all Go code with shared dependency resolution
- **Version Synchronization**: Single source of truth in go.mod ensures component consistency
- **Independent Evolution**: Client and server codebases evolve independently without coupling
- **Future-Proof**: Adding new SDKs (.NET, Ruby) requires no restructuring
- **Team Simplicity**: Flat Go layout easier to navigate for 5-engineer team

**Trade-offs:**

- **Multiple Packages to Manage**: More complex than single monolithic package (minimal overhead in monorepo)
- **Coordination Required**: Breaking changes to internal/types require coordinating across consumers
- **Module Configuration**: More complex go.mod (standard Go monorepo pattern)
- **Build Tooling Complexity**: Requires coordinating go build, Python tooling, Gradle (Java)
- **Large Repository**: All SDKs and Go code in single repository
- **Cross-Language CI**: CI pipelines must handle multiple language ecosystems

**Operational Implications:**

- Release process: Single workflow builds all artifacts from unified version
- Development setup: Developers need latest stable Go, Python, and Java toolchains
- CI/CD coordination: Build matrix tests Go core, Python SDK, Java SDK independently
- Binary distribution: Single trapperkeeper binary with embedded migrations
- SDK distribution: Python wheels to PyPI, Java JARs to Maven Central
- Version coupling: All components released together by design (intentional constraint)

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- Monorepo Structure: Maps to monorepo section, provides complete directory organization, build commands, version synchronization
- Package Separation: Maps to Go module architecture section, provides rationale for client/server split, SDK implications

**Dependencies** (foundational documents):

- [Principles: Architectural Principles](../01-principles/README.md): Simplicity principle informing modular package architecture (minimal needed for clean boundaries)
- [Architecture: Binary Distribution](../02-architecture/binary-distribution.md): Single binary architecture with subcommands

**Implements** (realizes these decisions):

- [Architecture: SDK Model](../02-architecture/sdk-model.md): SDK architecture with lean client-side dependencies
- [Architecture: Service Architecture](../02-architecture/README.md): Two-service model (web-ui, sensor-api) implemented by tk-core
- [Performance: Batch Processing and Vectorization](../05-performance/batch-processing.md): Pandas and Spark wrapper locations in SDK directories

**Extended by**:

- [Dependency Management](dependency-management.md): Go vendoring, Python/Java version pinning, upgrade policies
- Multi-language build automation (future document): Unified build scripts across Go/Python/Java
