---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: doc/02-architecture/README.md
tags:
  - two-service
  - grpc
  - http
  - protocol-separation
  - service-design
---

# Service Architecture & Protocol Separation

## Context

TrapperKeeper serves two distinct user groups: sensors requiring high-throughput machine communication, and human operators requiring web-based monitoring. This document specifies how the system separates these concerns into two independent services with optimized protocols for each use case.

**Hub Document**: This spoke is part of [Architecture Overview](README.md). See the hub's Two-Service Model section for strategic context on service separation and independent scaling.

## Two-Service Architecture

TrapperKeeper implements a clear separation between sensor communication and operator interfaces through two distinct services running as separate processes:

### Service Overview

**tk-sensor-api** (gRPC Service):

- **Purpose**: High-throughput sensor-to-server communication
- **Protocol**: gRPC using Protocol Buffers
- **Default Port**: 50051 (gRPC standard)
- **Optimizations**: Streaming for batch event submission, bidirectional communication for rule synchronization
- **Authentication**: HMAC-SHA256 API keys (see [Authentication (Sensor API)](../06-security/authentication-sensor-api.md))
- **Use Cases**: Rule synchronization requests, event batch ingestion from sensors

**tk-web-ui** (HTTP Service):

- **Purpose**: Operator interface for rule management and system monitoring
- **Protocol**: HTTP/HTTPS with server-side rendered HTML
- **Default Port**: 8080 (common HTTP)
- **UI Technology**: Go 1.22+ stdlib net/http with html/template
- **Authentication**: Cookie-based session authentication (see [Authentication (Web UI)](../06-security/authentication-web-ui.md))
- **Use Cases**: Rule creation/editing, event viewing, system configuration

### Key Architectural Principles

**Independent Processes**:

- Each service runs as a separate OS process with its own pidfile
- Services can be started, stopped, and restarted independently
- Pidfile locking prevents multiple instances of the same service

**Shared Database**:

- Both services access the same SQLite or PostgreSQL backend
- No inter-service communication: Services coordinate exclusively via database
- Schema migrations are shared and version-controlled

**Single Binary Distribution**:

- Both services packaged as subcommands in a single `trapperkeeper` binary
- Unified versioning and release process
- Shared internal packages reduce code duplication

**Example Usage**:

```bash
# Terminal 1: Start sensor API
./trapperkeeper sensor-api --port 50051 --data-dir /var/lib/trapperkeeper

# Terminal 2: Start web UI
./trapperkeeper web-ui --port 8080 --data-dir /var/lib/trapperkeeper
```

Both services read/write to the same database at `/var/lib/trapperkeeper/trapperkeeper.db`.

## Protocol Separation

Each service uses a protocol optimized for its specific audience:

### Sensor Communication (gRPC)

**Protocol Benefits**:

- Protocol Buffers provide schema evolution support
- Efficient binary serialization reduces bandwidth consumption
- Streaming support enables batch event submission
- Bidirectional communication for real-time rule synchronization
- Type-safe contracts enforced at compile time

**Design Rationale**: Machine-to-machine communication prioritizes performance, efficiency, and type safety. gRPC provides these characteristics while maintaining a clean contract between sensors and server.

**Key Features**:

- ETAG-based rule synchronization minimizes network overhead
- Batch event submission reduces round-trip latency
- Binary encoding optimizes for high-throughput scenarios

**Cross-References**:

- [API Service Architecture](api-service.md): Complete gRPC protocol specification
- [SDK Model](sdk-model.md): Client-side SDK patterns for gRPC communication

### Operator Interface (HTTP/HTML)

**Protocol Benefits**:

- Traditional request-response model familiar to web developers
- Server-side rendered HTML requires no JavaScript
- Form-based interactions with CSRF protection
- Human-readable URLs and error messages
- Browser-based interaction without custom tooling

**Design Rationale**: Human operators need accessibility and clarity over performance. HTTP/HTML provides universal browser compatibility, simple debugging, and straightforward deployment.

**Key Features**:

- No JavaScript required (progressive enhancement philosophy)
- Standard HTTP verbs for CRUD operations
- Cookie-based session management
- Traditional form submissions with POST-Redirect-GET pattern

**Cross-References**:

- [Web Framework](../09-operations/web-framework.md): Go net/http service implementation

## TLS Integration

Both services support TLS for transport security with flexible deployment modes:

### tk-web-ui TLS Strategy

**Direct TLS Termination**:

- HTTPS mode with `--tls-cert-file` and `--tls-key-file` CLI flags
- TLS 1.3 minimum with modern cipher suites
- Secure cookie flag always enabled in HTTPS mode

**Reverse Proxy Mode** (Recommended):

- Service runs in HTTP mode, reverse proxy (nginx, Caddy, AWS ALB) terminates TLS
- Custom HTTP middleware detects `X-Forwarded-Proto: https` header
- Secure cookie flag automatically enabled when HTTPS detected

**Development Mode**:

- HTTP without TLS for local development
- Secure cookie flag disabled
- Clear warning logged on startup

### tk-sensor-api TLS Strategy

**Direct TLS Termination**:

- gRPC with TLS using `--grpc-tls-cert-file` and `--grpc-tls-key-file` CLI flags
- TLS 1.3 minimum (same cipher suites as Web UI)
- Defense in depth: TLS secures transport, HMAC authenticates requests

**Plaintext Mode** (Development Only):

- gRPC without TLS for local testing
- HMAC authentication still required
- Clear warning logged on startup

**Certificate Management**:

- Separate certificates for each service (different ports, different domains)
- Manual certificate provisioning and renewal
- Startup validation checks certificate expiry and key matching

**Cross-References**:

- [TLS/HTTPS Strategy](../06-security/tls-https-strategy.md): Complete TLS configuration, certificate management, and reverse proxy examples

## Health Check Endpoints

Both services provide standardized health check endpoints for container orchestration and monitoring:

### Standard Endpoints

**Liveness Check** (`/healthz`):

- **Purpose**: Indicates service process is alive and not deadlocked
- **Response**: 200 OK with `{"status": "ok"}`
- **Timeout**: Must respond within 1 second
- **Failure Behavior**: Kubernetes restarts pod after 3 consecutive failures

**Readiness Check** (`/readyz`):

- **Purpose**: Indicates service is ready to receive traffic (database connectivity confirmed)
- **Response**: 200 OK with `{"status": "ready"}` or 503 with error details
- **Database Check**: Ping database with 1-second timeout
- **Failure Behavior**: Kubernetes removes pod from service endpoints after 3 consecutive failures

### Prometheus Metrics

**tk-sensor-api Only**:

- **Endpoint**: `/api/v1/stats/prometheus`
- **Format**: Prometheus text exposition format
- **Authentication**: None (MVP, internal network assumption)
- **Metrics**: Event counts by action, rule count, storage size

**Example Metrics**:

```
trapperkeeper_events_received_total{action="observe"} 12345
trapperkeeper_events_received_total{action="drop"} 678
trapperkeeper_rule_count 42
trapperkeeper_storage_size_bytes 123456789
```

**Cross-References**:

- [Health Check Endpoints](../09-operations/health-endpoints.md): Complete health check specification, Kubernetes configuration examples, Prometheus scrape configuration

## Scaling Strategy

Services can scale independently based on load characteristics:

### Horizontal Scaling

**Sensor API Scaling**:

- Multiple `tk-sensor-api` instances behind load balancer
- Stateless protocol enables simple round-robin distribution
- Database connection pool shared across instances

**Web UI Scaling**:

- Multiple `tk-web-ui` instances for operator access
- Session store backed by database or Redis
- Lower scaling requirements (fewer operators than sensors)

### Vertical Scaling

**Resource Allocation**:

- Sensor API: CPU-bound (rule evaluation, event parsing)
- Web UI: Memory-bound (template rendering, session management)
- Independent resource tuning per service

**Kubernetes Resource Limits** (example):

```yaml
# Sensor API: CPU-optimized
resources:
  requests:
    cpu: "2000m"
    memory: "512Mi"
  limits:
    cpu: "4000m"
    memory: "1Gi"

# Web UI: Memory-optimized
resources:
  requests:
    cpu: "500m"
    memory: "1Gi"
  limits:
    cpu: "1000m"
    memory: "2Gi"
```

## Database Coordination

Both services share the same database backend, requiring coordination:

### Schema Management

**Database Migrations**:

- Single set of migrations applied via `trapperkeeper migrate` subcommand
- Both services must use compatible schema versions
- Migration versioning prevents schema drift

**Connection Pooling**:

- Each service maintains its own connection pool
- Pool sizes tuned independently based on service load
- Database backend handles concurrent access from both services

### Operational Constraints

**Single-Instance Enforcement**:

- Pidfile locking prevents multiple instances of same service on one host
- Kubernetes deployments manage multiple instances across different hosts
- No distributed locking required (database provides consistency)

**Deployment Order**:

1. Run `trapperkeeper migrate` to apply schema changes
2. Start `tk-sensor-api` instances
3. Start `tk-web-ui` instances

**Cross-References**:

- [Database Backend](../09-operations/database-backend.md): Database selection, connection pooling, migration strategy

## Benefits and Trade-offs

### Benefits

**Clear Separation of Concerns**:

- Sensor communication decoupled from operator interfaces
- Protocol optimization for specific use cases (machines vs. humans)
- Independent development and testing of each service

**Operational Simplicity**:

- Two services instead of dozens (microservices avoided)
- Unified binary distribution (single artifact to deploy)
- Shared database simplifies data consistency

**Independent Scaling**:

- Scale gRPC service based on sensor traffic
- Scale HTTP service based on operator usage
- Different resource profiles per service

**Protocol Flexibility**:

- Can change Web UI without affecting sensor protocol
- gRPC protocol evolution independent of HTTP changes
- Separate TLS certificates and renewal schedules

### Trade-offs

**Two Processes to Manage**:

- Requires process orchestration (systemd, Kubernetes, Docker Compose)
- Health checks and monitoring for both services
- Log aggregation must distinguish between services

**Database as Shared Dependency**:

- Database becomes potential bottleneck
- Schema changes must coordinate across services
- Connection pool contention under high load

**No Direct Inter-Service Communication**:

- Services coordinate via database only (intentional constraint)
- Cannot leverage request-response patterns between services
- All state shared through database tables

## Related Documents

**Hub Document**:

- [Architecture Overview](README.md): Strategic overview of two-service model, SDK design, and binary distribution

**Dependencies** (read these first):

- [Principles Architecture](../01-principles/README.md): Simplicity and schema-agnostic principles informing two-service design
- [Database Backend](../09-operations/database-backend.md): Shared database backend used by both services

**Related Spokes** (siblings in this hub):

- [API Service Architecture](api-service.md): Complete gRPC protocol specification for tk-sensor-api
- [SDK Model](sdk-model.md): Client-side SDK patterns for sensor communication
- [Binary Distribution Strategy](binary-distribution.md): Single binary build and packaging approach

**Security Integration**:

- [TLS/HTTPS Strategy](../06-security/tls-https-strategy.md): TLS configuration, certificate management, reverse proxy setup
- [Authentication (Sensor API)](../06-security/authentication-sensor-api.md): HMAC-based API authentication
- [Authentication (Web UI)](../06-security/authentication-web-ui.md): Cookie-based session authentication

**Operational Integration**:

- [Health Check Endpoints](../09-operations/health-endpoints.md): Health checks, readiness probes, Prometheus metrics
- [Web Framework](../09-operations/web-framework.md): Go net/http service implementation
- [Configuration Management](../09-operations/configuration.md): CLI flags, environment variables, startup validation
