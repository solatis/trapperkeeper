# ADR-005: API Service Architecture

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |
| 2025-10-30 | Added backlink to ADR-018 (Rule Lifecycle) |

## Context

TrapperKeeper requires an API layer to enable communication between ephemeral sensors and the central coordination server, using efficient binary protocol for high-performance rule checking and stateless operation for network resilience.

## Decision

We will implement **two distinct services** with different protocols:

### 1. Service Separation

- **`tk-sensor-api`**: gRPC service for sensor communication (dedicated port)
- **`tk-web-ui`**: HTTP service for web interface with server-side rendering (separate port)
- Both services share the same SQLite database
- Single API instance enforced via pidfile/lockfile mechanism (MVP constraint)

**Rationale**: Clean separation between machine protocol (gRPC) and human interface (HTTP). Different ports allow independent scaling and security policies.

### 2. gRPC Protocol for Sensors

All sensor API calls use **gRPC** (not HTTP/JSON REST):

- **Protocol buffers** provide strong type contracts
- **Binary encoding** reduces bandwidth and parsing overhead
- **Streaming capabilities** support future async event reporting
- **Code generation** ensures type-safe clients across languages (Python, Java, Go)
- **Built-in retry/timeout** semantics simplify SDK implementation

**Core API Operations**:
- `SyncRules()`: Sensors fetch applicable rules filtered by tags
- `ReportEvents()`: Sensors submit matched events in batches

**Protocol Design Highlights**:
- DNF (Disjunctive Normal Form) rules with OR-of-ANDs structure
- Server-side rule filtering by sensor tags
- ETAG-based conditional sync (avoid re-fetching unchanged rules)
- Event batching with client-generated UUIDv7 identifiers
- Rule transformations applied server-side (e.g., dry-run mode converts actions to "observe")

### 3. Stateless Protocol Design

**No persistent connections**:
- Sensors sync rules periodically (default: 30 seconds, configurable)
- ETAG mechanism enables conditional sync (304-equivalent via empty response)
- In-memory cache per sensor (no disk cache in MVP)
- Network partition handling via configurable fail-safe mode (default: disable all rules when offline)

**Benefits**:
- Simpler SDK implementation (no connection management)
- Naturally handles ephemeral sensors (destroyed at job end)
- Server doesn't track sensor state or health
- Aligns with "least intrusive by default" principle

### 4. Event Ingestion Model

**Batch-synchronous reporting** (MVP):
- Events buffered in-memory during rule evaluation
- Synchronous gRPC call when buffer reaches limit or explicitly flushed
- Buffer limits: 10,000 events per sensor, 1MB per event, 100MB total memory
- Clear error when buffer full: "Event buffer full, flush required"
- **No auto-flush** (explicit > implicit principle)

**Flush Triggers**:
- After checking all rules against entire dataset (batch completion)
- Explicit `sensor.flush_events()` call
- Context manager exit (`__exit__` auto-flushes)

**Error Handling**:
- Never return errors on partial event failures (observability issues shouldn't fail pipelines)
- Client logs warning, continues processing
- Future: async batched reporting with DLQ

### 5. Rule Management

**Server-side intelligence**:
- Rules filtered by sensor tags (sensors only receive applicable rules)
- Priority calculated dynamically based on complexity scoring:
  - Field path depth (10 points per segment)
  - Wildcard multipliers (assume 8x evaluations)
  - Operator costs (1-4 points: integer < text < type coercion)
- Transformations applied (dry-run mode converts all actions to "observe")

**Rule Synchronization**:
- Sensors provide tags in `SyncRulesRequest`
- Server computes ETAG from filtered rule set
- ETAG cached in-memory on client
- Subsequent syncs include ETAG to avoid refetch

## Consequences

### Benefits

1. **Performance**: gRPC binary protocol minimizes serialization overhead for high-throughput sensors
2. **Type Safety**: Protobuf schemas catch integration errors at compile time
3. **Language Support**: Code generation simplifies multi-language SDK development (Python, Java, Go)
4. **Future-Proof**: gRPC streaming enables async event reporting without protocol changes
5. **Clear Boundaries**: Service separation allows independent deployment, monitoring, and security policies
6. **Resilient**: Stateless design handles network partitions gracefully without complex reconnection logic
7. **Developer Experience**: Explicit buffer management prevents hidden memory issues

### Tradeoffs

1. **Complexity**: Two services to deploy, configure, and monitor (vs single HTTP service)
2. **Protobuf Learning Curve**: Developers must understand protobuf schema evolution
3. **Debugging**: Binary protocol harder to inspect than JSON (mitigated by gRPC reflection, tools like grpcurl)
4. **Synchronous Bottleneck**: Batch-synchronous event reporting blocks pipeline during POST (future async optimization planned)
5. **Single Instance Constraint**: Lockfile enforcement prevents horizontal scaling (temporary MVP limitation)

### Operational Implications

1. **Port Management**: Requires two ports open (e.g., 8080 for HTTP, 50051 for gRPC)
2. **Load Balancing**: gRPC requires L7 load balancer with HTTP/2 support
3. **TLS Configuration**: gRPC and HTTP require separate TLS certificates
4. **Monitoring**: Different metrics/logs for gRPC (request/response sizes, connection pooling) vs HTTP
5. **Database Sharing**: Both services access same SQLite file (concurrency handled by SQLite locking)

## Implementation

1. Define protobuf schema with:
   - `SyncRules` RPC with tag-based filtering
   - `ReportEvents` RPC with batch support
   - DNF rule representation (Rule → AndGroup → Condition)
   - ETAG support via gRPC metadata

2. Implement `tk-sensor-api` service:
   - gRPC server with API key authentication (x-api-key metadata)
   - Rule filtering and priority calculation
   - ETAG generation from rule set hash
   - Event ingestion with buffering

3. Implement `tk-web-ui` service:
   - HTTP server with server-side rendering
   - Rule management CRUD operations
   - Event search interface
   - Shared database access with sensor API

4. Enforce single instance via pidfile:
   - Write PID to lockfile on startup
   - Check for stale locks (process dead)
   - Exit with error if active instance detected

5. SDK implementation:
   - Generated protobuf clients (Python, Java, Go)
   - In-memory rule cache with ETAG storage
   - Event buffer management with explicit flush
   - Configurable fail-safe/fail-closed modes

## Related Decisions

This ADR implements the Schema-Agnostic Architecture and Ephemeral Sensors principles by providing a stateless gRPC protocol with no schema validation at the API layer.

**Depends on**:
- **ADR-001: Architectural Principles** - Implements Schema-Agnostic Architecture (no schema validation) and Ephemeral Sensors (stateless protocol) principles

**Extended by**:
- **ADR-006: Service Architecture** - Separates into tk-sensor-api and tk-web-ui services
- **ADR-012: API Authentication** - Defines HMAC-based authentication for sensor API
- **ADR-018: Rule Lifecycle and Operational Controls** - Uses ETAG-based sync mechanism for rule distribution

## Future Considerations

- **Horizontal scaling**: Remove single instance constraint via distributed locking
- **Async event reporting**: Background streaming for non-blocking event ingestion
- **gRPC load balancing**: Connection pooling and client-side load balancing strategies
- **Bi-directional streaming**: Real-time rule updates pushed to sensors
- **Event compression**: Protocol-level compression for high-volume sensors
