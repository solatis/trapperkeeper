---
doc_type: hub
status: active
date_created: 2025-11-07
primary_category: architecture
consolidated_spokes:
  - api-service.md
  - sdk-model.md
  - service-architecture.md
  - binary-distribution.md
  - ../10-integration/README.md
  - ../10-integration/package-separation.md
  - ../10-integration/monorepo-structure.md
tags:
  - two-service
  - grpc
  - http
  - sdk
  - integration
  - monorepo
---

# Architecture Overview

## Context

TrapperKeeper's architecture evolved from a Complexity is NOT a feature for complex systems, build minimal systems that leverage existing tools, avoid inventing when existing solutions exist well principle established in Principles Architecture. The system requires clear boundaries between sensor communication (gRPC for performance) and operator interfaces (HTTP for accessibility), while maintaining simplicity through unified distribution and developer-friendly SDKs.

Traditional monolithic architectures combine all concerns in single service, creating deployment coupling and making independent scaling impossible. Microservice architectures fragment systems into dozens of services, introducing operational complexity unsuitable for five-engineer teams. TrapperKeeper needs a middle ground: enough separation for clear boundaries, minimal services for operational simplicity.

This hub consolidates architectural decisions establishing TrapperKeeper's two-service model, SDK development approach, and unified binary distribution strategy.

## Decision

We implement a **two-service architecture** with `tk-sensor-api` (gRPC) handling high-throughput sensor communication and `tk-web-ui` (HTTP) providing operator interfaces. Both services are packaged as subcommands in a single binary enabling unified distribution while maintaining service independence.

This document serves as the architectural hub providing strategic overview with cross-references to detailed implementation documents for API service design, SDK model, and binary distribution.

### Two-Service Model

TrapperKeeper separates concerns into two independent services sharing database backend:

**tk-sensor-api** (gRPC Service):

- Handles sensor-to-server communication via gRPC
- Stateless protocol with ETAG-based rule synchronization
- HMAC-based authentication for API security
- Designed for high-throughput event ingestion
- Default port: 50051 (gRPC standard)

**tk-web-ui** (HTTP Service):

- Provides operator interface for rule management
- Server-side rendered HTML using net/http and html/template
- Cookie-based session authentication
- CSRF protection for state-changing operations
- Default port: 8080 (common HTTP)

**Key Principles**:

- Independent processes: Each service runs as separate OS process with own pidfile
- Shared database: Both services access same SQLite/PostgreSQL backend
- Single-instance enforcement: Pidfile locking prevents multiple instances per service
- No inter-service communication: Services coordinate via database, not RPC
- Unified distribution: Both services packaged as subcommands in single binary

**Cross-References**:

- API Service Architecture: Complete gRPC protocol specification
- Binary Distribution Strategy Section 1: Subcommand implementation
- SDK Model Section 2: Client-side API communication patterns

**Example**:

Two separate processes sharing database:

```bash
# Terminal 1: Start sensor API
./trapperkeeper sensor-api --port 50051 --data-dir /var/lib/trapperkeeper

# Terminal 2: Start web UI
./trapperkeeper web-ui --port 8080 --data-dir /var/lib/trapperkeeper
```

Both services read/write same database at `/var/lib/trapperkeeper/trapperkeeper.db`.

### Protocol Boundaries

Clear protocol boundaries enable independent evolution:

**Sensor Communication (gRPC)**:

- Protocol Buffers for schema evolution support
- Streaming for batch event submission
- Bidirectional communication for rule sync
- Efficient binary serialization reducing bandwidth
- Designed for machine-to-machine communication

**Operator Interface (HTTP/HTML)**:

- Traditional request-response model
- Server-side rendered HTML (no JavaScript required)
- Form-based interactions with CSRF protection
- Human-readable URLs and error messages
- Designed for browser-based interaction

**Rationale**: Separate protocols optimize for different audiences. gRPC provides performance for high-throughput sensors. HTTP/HTML provides accessibility for operators without requiring API clients or custom tooling.

**Cross-References**:

- API Service Architecture Sections 2-3: Complete protocol buffer schemas
- Web Framework Selection: HTTP service implementation with net/http

### Developer-First SDK Design

SDKs prioritize developer experience through pre-compilation, explicit buffer management, and fail-safe defaults:

**Pre-Compiled Rules**:

- Rules compiled on server, synchronized to sensors
- SDK receives pre-compiled rule bytecode, not raw expressions
- Zero parsing overhead during event evaluation
- Simplifies SDK implementation across languages

**Explicit Buffer Management**:

- Developers control when events are sent via `flush()`
- No hidden background threads or automatic flushing
- Predictable memory usage and network behavior
- Clear error boundaries for failure handling

**Ephemeral Sensors**:

- Sensors live for duration of workload (minutes to hours)
- No persistent registration or long-lived connections
- Aligns with Airflow DAGs, Spark jobs, Kubernetes pods
- Simplifies cleanup and reduces operational complexity

**Fail-Safe Defaults**:

- Fallback to cached rules when API unreachable
- Configurable failure modes: fail-open, fail-closed, fail-safe
- Silent degradation for non-critical failures
- Least intrusive by default per Principle #2

**Key Benefits**:

- Low ceremony: Minimal boilerplate for common use cases
- Predictable behavior: No hidden magic or surprising defaults
- Fail-safe: Degraded operation better than complete failure
- Language-native: Each SDK feels natural in its ecosystem

**Cross-References**:

- SDK Model: Complete SDK design philosophy and patterns
- Failure Modes and Degradation: Fail-safe strategy details
- Batch Processing and Vectorization: Performance optimizations

**Example SDK Usage**:

```python
from trapperkeeper import Sensor

# Initialize sensor (ephemeral, lives for job duration)
sensor = Sensor(api_key=os.environ['TK_API_KEY'])

# Process records (buffered locally)
for record in data_source:
    sensor.observe(record)

# Explicit flush (developer controls timing)
sensor.flush()
```

### Unified Binary Distribution

Single binary contains both services as subcommands, simplifying deployment:

**Binary Structure**:

```
trapperkeeper               # Main binary (~15-20MB)
├── sensor-api              # Subcommand for gRPC service
├── web-ui                  # Subcommand for HTTP service
└── migrate                 # Subcommand for database migrations
```

**Deployment Benefits**:

- Single artifact to distribute and version
- Consistent tooling across services
- Shared database migration logic
- Reduced operational complexity
- No coordination of multiple binaries

**Build Strategy**:

- Single Go module (go.mod) with internal/ packages
- Static binary builds (CGO_ENABLED=0) for size and portability
- Single version number across all components
- Embedded database migrations at compile time (embed.FS)

**Cross-References**:

- Binary Distribution Strategy: Complete build and packaging approach
- Client/Server Package Separation: Internal package architecture enabling unified binary
- Monorepo Directory Structure: Go module organization

**Example Deployment**:

```bash
# Single binary download
curl -L https://releases.trapperkeeper.ai/trapperkeeper-v0.1.0-linux-x86_64 -o trapperkeeper
chmod +x trapperkeeper

# Run migrations once
./trapperkeeper migrate --db-url postgres://localhost/trapperkeeper

# Start both services (separate processes)
./trapperkeeper sensor-api --port 50051 &
./trapperkeeper web-ui --port 8080 &
```

## Consequences

**Benefits:**

- Clear separation of concerns: Sensor communication decoupled from operator interfaces
- Independent scaling: Scale gRPC service separately from HTTP service based on load
- Protocol optimization: Each protocol optimized for its audience (machines vs humans)
- Operational simplicity: Two services instead of dozens, unified binary distribution
- Development velocity: Clear boundaries enable parallel development without conflicts
- Fail-safe architecture: Services degrade gracefully when dependencies unavailable

**Trade-offs:**

- Two processes to manage instead of one (acceptable for clear boundaries)
- Database becomes shared dependency and potential bottleneck
- No direct inter-service communication (intentional constraint)
- Must coordinate schema changes across services (mitigated by migrations)

**Operational Implications:**

- Container orchestration must manage two services
- Database connection pool shared between services
- Pidfile enforcement prevents accidental multi-instance deployment
- Health checks required for both services (see Operational Endpoints)
- Log aggregation must distinguish between services

## Related Documents

**Consolidated Spokes** (this hub consolidates):

- API Service Architecture: Maps to gRPC protocol section above, provides complete protocol buffer schemas
- SDK Model: Maps to developer-first SDK design section above, provides implementation patterns
- Binary Distribution Strategy: Maps to unified binary section above, provides build configuration

**Dependencies** (foundational documents):

- Principles Architecture: Establishes simplicity and schema-agnostic principles informing two-service design
- Database Backend: Defines shared database backend used by both services

**References** (related hubs/documents):

- Data Hub: Event schema and storage patterns used by both services
- Operations Hub: Configuration, CLI design, and operational endpoints
- Security Architecture: Authentication strategies (HMAC for API, cookies for Web UI)

**Extended by**:

- TLS/HTTPS Strategy: Security configuration for both HTTP and gRPC services
- Service Health Endpoints: Health check implementation for both services
