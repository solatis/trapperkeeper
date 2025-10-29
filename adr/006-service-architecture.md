# ADR-006: Service Architecture & Protocol Separation

Date: 2025-10-28

## Related Decisions

**Depends on:**
- **ADR-005: API Service Architecture** - Builds upon the gRPC/HTTP protocol separation to create two distinct services

**Extended by:**
- **ADR-007: CLI Configuration** - Configures both tk-sensor-api and tk-web-ui services
- **ADR-008: Web Framework** - Implements HTTP service for tk-web-ui
- **ADR-009: Operational Endpoints** - Adds health checks to both services

## Context

TrapperKeeper serves two distinct user groups with different needs: sensors (programmatic clients requiring high-throughput machine communication) and human operators (requiring web-based monitoring and configuration). Combining these into a single service would mix concerns and protocols in ways that complicate deployment and scaling.

We need clean separation between:
- Sensor communication protocol optimized for efficiency and type safety
- Human interface optimized for accessibility and ease of use

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

Both services share the same database and event storage but operate independently.

## Consequences

**Benefits:**
- Clean separation between sensor protocol (gRPC) and human interface (HTTP)
- Each service optimized for its specific use case
- Independent scaling: API server can scale separately from web UI
- Protocol flexibility: Can change web UI without affecting sensor protocol
- Simpler testing: Can test each service in isolation

**Tradeoffs:**
- Two separate binaries to deploy and manage
- Shared database access requires coordination
- Need to ensure both services have compatible schema expectations
- Additional operational complexity compared to single service

This architecture provides clear boundaries while maintaining simplicity through shared data storage.
