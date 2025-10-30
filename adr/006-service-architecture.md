# ADR-006: Service Architecture & Protocol Separation

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper serves two distinct user groups: sensors requiring high-throughput machine communication, and human operators requiring web-based monitoring. Combining these into a single service would mix concerns and protocols in ways that complicate deployment and scaling.

## Decision

Split the system into two distinct services running on separate ports:

- **`tk-sensor-api`**: gRPC service for sensor communication
  - Handles rule synchronization requests from sensors
  - Accepts event batches from sensors
  - Optimized for high-throughput, low-latency communication
  - Uses Protocol Buffers for type-safe, efficient serialization

- **`tk-web-ui`**: HTTP service for web interface
  - Provides server-side rendered HTML interface
  - Allows operators to manage rules, view events, configure system
  - Uses standard HTTP with traditional form submissions
  - No JavaScript required in MVP

Both services are built from a single binary using a cargo workspace structure. The binary includes a core library (`tk-core`) shared between both services. Services are launched using subcommands (e.g., `trapperkeeper sensor-api`, `trapperkeeper web-ui`).

Both services share the same database and event storage but operate independently. Despite the single binary, services still run as separate processes and can scale independently by running multiple instances with different subcommands.

## Consequences

**Benefits:**
- Clean separation between sensor protocol (gRPC) and human interface (HTTP)
- Each service optimized for its specific use case
- Independent scaling: API server can scale separately from web UI
- Protocol flexibility: Can change web UI without affecting sensor protocol
- Simpler testing: Can test each service in isolation
- Single binary simplifies build and release process (one build, one version)
- Shared core library reduces code duplication for database logic

**Tradeoffs:**
- Two separate service processes to manage despite single binary
- Shared database access requires coordination
- Need to ensure both services have compatible schema expectations
- Additional operational complexity compared to single monolithic service

**Operational Implications:**
- Two separate processes must be managed and monitored
- Database connection pooling must be configured for both services
- Both services must be deployed and started for full system operation

This architecture provides clear boundaries while maintaining simplicity through shared data storage.

## Implementation

1. Create `tk-core` library crate with shared database and business logic
2. Create `tk-sensor-api` binary crate implementing gRPC service using tonic
3. Create `tk-web-ui` binary crate implementing HTTP service using Axum
4. Configure cargo workspace to build single binary with subcommands (`sensor-api`, `web-ui`)
5. Configure separate ports for each service (default: 50051 for gRPC, 8080 for HTTP)
6. Implement shared database connection management in `tk-core`

## Related Decisions

**Depends on:**
- **ADR-005: API Service Architecture** - Builds upon the gRPC/HTTP protocol separation to create two distinct services

**Extended by:**
- **ADR-007: CLI Configuration** - Configures both tk-sensor-api and tk-web-ui services
- **ADR-008: Web Framework** - Implements HTTP service for tk-web-ui
- **ADR-009: Operational Endpoints** - Adds health checks to both services
