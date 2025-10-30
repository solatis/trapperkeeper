# ADR-002: SDK Model

## Revision log

| Date | Description |
|------|-------------|
| 2025-10-28 | Document created |

## Context

TrapperKeeper requires a mechanism for integrating rule evaluation into data processing pipelines. Traditional approaches (sidecar proxies, agents, infrastructure plugins) add deployment complexity and hide integration points from developers.

## Decision

We will implement a **developer-first SDK model** where TrapperKeeper is integrated as a library, not infrastructure.

### 1. Library Integration Model

**Not infrastructure**:
- No sidecars, proxies, or agents to deploy
- No invisible instrumentation or bytecode manipulation
- No persistent daemons or background processes

**Explicit library imports**:
- Language-native SDKs imported as dependencies (Python, Java, Go)
- Sensors visible in user code, requiring explicit integration
- Developers see and control when/where rules are evaluated

**Rationale**: Explicit integration prevents "magic" that breaks during debugging. Engineers understand data flow because they wrote it. No hidden infrastructure to troubleshoot when pipelines fail.

### 2. Ephemeral Sensor Architecture

**Sensors are stateless**:
- Created at job start, destroyed at job end
- No persistent identity or registration
- State limited to in-memory rule cache and event buffer
- Tied to process lifecycle (minutes to hours)

**No concept of "sensor health"**:
- Server does not track which sensors are online
- Operators view sensors through events: "what reported in the past hour?"
- No heartbeats, no persistent connections

**Rationale**: Aligns with modern ephemeral compute (containers, serverless, batch jobs). Avoids state management complexity. Sensors naturally disappear when jobs complete.

### 3. Language-Native Base SDKs

**Core Rust library**:
- `trapperkeeper-core` - Core Rust SDK library provides rule parsing, gRPC communication, and evaluation logic
- All heavy lifting (rule parsing, sensor-api communication) centralized in Rust
- Language-specific SDKs become lightweight wrappers over the core library

**Language bindings**:
- `sdks/python/` - Python SDK built with pyo3 for native performance
- `sdks/java/` - Java SDK using JNI bindings

**Framework wrappers built on base**:
- Python → Pandas API, Airflow operators
- Java → Spark transformations
- Framework wrappers override `api_type` metadata (Airflow wrapper reports `"airflow"`, not `"python"`)

**Rationale**: Core Rust library reduces duplication across language SDKs. All heavy lifting centralized in Rust achieves optimal performance with nanosecond-level evaluation. pyo3 enables native Rust extensions for Python, achieving significantly better performance than ctypes or cffi approaches. Native Python extensions via pyo3 eliminate Python/C boundary overhead. Framework wrappers leverage existing base SDKs rather than reimplementing protocol logic.

### 4. Pre-Compilation for Performance

**SDKs operate on parsed data structures**:
- Not raw bytes or wire formats
- Leverage existing parsers (Pandas reads Parquet, Spark reads Avro)
- Field paths resolved against in-memory objects/DataFrames

**Rules compiled to native predicates**:
- Rust: Compiled to concrete enum with pattern matching
- Zero virtual dispatch, nanosecond-level pattern matching
- Compiler can inline aggressively for optimal performance
- Compilation happens once when rules fetched from API

**Rationale**: Pre-compilation amortizes parsing cost across thousands/millions of records. Enables <1ms evaluation target. Avoids JSON parsing on hot path.

### 5. Explicit Buffer Management

**No auto-flush**:
- Events buffered in memory during processing
- Explicit `sensor.flush_events()` call sends to API
- Context manager pattern auto-flushes on exit

**Buffer limits**:
- 10,000 events per sensor
- 1MB maximum per event
- 100MB total memory per sensor

**Clear errors when full**:
- "Event buffer full, flush required"
- Forces developer awareness of memory usage

**Rationale**: "Explicit > implicit" principle. Auto-flush hides memory pressure and network I/O timing. Developers control when network calls happen.

### 6. Metadata Collection Strategy

**One-time detection at startup**:
- Hostname, IP, process ID, OS details collected once
- Cached for process lifetime
- Environment variables (`TK_META_*`) auto-collected
- Kubernetes detection via env vars or service account files

**Standard metadata**:
- `ip`, `hostname`, `process_id`, `thread_id`
- `api_type`, `api_version`
- `os_type`, `os_version`
- Optional: `k8s_pod_name`, `k8s_pod_namespace`

**User metadata limits**:
- Max 50 key-value pairs
- 128 char keys, 1KB values
- 64KB total per sensor

**Rationale**: One-time collection minimizes overhead for short-lived sensors. Environment variable pattern enables deployment-time configuration without code changes. Limits prevent unbounded memory growth.

### 7. Configurable Failure Modes

**Fail-safe by default** (offline = disabled):
- When API unreachable: disable all rules
- Operates as pass-through (no observe/drop/error actions)
- Prevents pipeline failures from network issues

**Alternative modes** (configurable per sensor):
- Fail-closed: Error when offline
- Fail-open with cache: Use cached rules with TTL

**Rationale**: "Least intrusive by default" principle. Observability system should never break production pipelines. Engineers explicitly opt into strict mode when needed.

### 8. Sensor Tag System

**Tags enable rule targeting**:
- Alphanumeric + dash/underscore/dot/colon
- Max 128 chars per tag
- Case-sensitive exact matching
- Examples: `production`, `customer-123`, `region:us-west-2`

**Rules filtered by tags**:
- Sensors declare tags at initialization
- API returns only applicable rules
- No wildcard matching (exact match only)

**Rationale**: Simple tag system enables environment-specific rules (prod vs dev) and data-type-specific rules (PII vs metrics) without complex routing logic.

## Consequences

### Benefits

1. **No Infrastructure Deployment**: Engineers add SDK dependency, no containers/agents/proxies to deploy
2. **Explicit Integration**: Sensor lifecycle visible in code, easier to debug and understand
3. **Performance**: Pre-compilation and in-memory evaluation achieves <1ms target
4. **Framework Flexibility**: Base SDKs support arbitrary frameworks via thin wrappers
5. **Ephemeral-Friendly**: Stateless design aligns with modern container/serverless patterns
6. **Fail-Safe Defaults**: Network issues degrade to pass-through, preventing pipeline failures
7. **Developer Control**: Explicit buffer management prevents hidden memory/latency issues

### Tradeoffs

1. **Code Changes Required**: Unlike transparent agents, requires modifying data pipeline code
2. **SDK Maintenance Burden**: Must maintain compatibility across Python/Java/Go ecosystems
3. **Framework Proliferation**: Every new framework (Flink, Beam, Dagster) needs wrapper
4. **Version Skew**: Different jobs may use different SDK versions with varying behavior
5. **Memory Overhead**: Each sensor instance holds rule cache and event buffer (100MB+)
6. **No Retroactive Integration**: Cannot observe legacy pipelines without code changes

### Operational Implications

1. **Deployment Model**: SDK version distributed via package managers (pip, Maven, Go modules)
2. **Rollout Strategy**: New rules propagate via API sync, no SDK redeployment needed
3. **Debugging**: Sensor logs appear in application logs, not separate infrastructure logs
4. **Monitoring**: Sensor metrics reported through API events, not separate telemetry system
5. **Upgrades**: SDK upgrades require application redeployment, not infrastructure changes

## Implementation

1. Define gRPC protocol for sensor-API communication (see ADR-004)

2. Implement base SDKs with:
   - Rule synchronization with ETAG-based conditional fetch
   - Pre-compilation to native predicates
   - In-memory event buffering with explicit flush
   - Context manager pattern for automatic lifecycle management
   - Configurable fail-safe/fail-closed modes

3. Implement metadata collection:
   - One-time detection at library import
   - Environment variable scanning (`TK_META_*`)
   - Kubernetes detection via service account files
   - Graceful degradation if detection fails

4. Create framework wrappers:
   - Pandas: Vectorized DataFrame operations
   - Airflow: Operator and sensor primitives
   - Spark: DataFrame transformation functions
   - Each wrapper overrides `api_type` metadata

5. Enforce buffer limits:
   - Count-based limit (10K events)
   - Per-event size limit (1MB)
   - Total memory limit (100MB)
   - Clear error messages when exceeded

## Related Decisions

This ADR implements the Ephemeral Sensors principle from ADR-001 through library-based integration and stateless sensor architecture. ADR-023 extends this model with vectorized operations for data frameworks.

**Depends on**:
- **ADR-001** - Architectural Principles

**Extended by**:
- **ADR-023** - Batch Processing and Vectorization

## Future Considerations

- **Async event reporting**: Background async task for non-blocking POST
- **Dead letter queue**: Persistent buffer for events that fail to send
- **Automatic SDK updates**: In-process hot reload of rule engine without restart
- **Telemetry streaming**: Real-time sensor health metrics independent of events
- **Bytecode instrumentation**: Optional transparent mode for frameworks that support it

## Appendix A: Standard Metadata Collection

### Always Collected
SDKs automatically collect these fields at startup:
- `ip`: IP address of device handling default route (probe `1.1.1.1` if needed)
- `hostname`: System hostname
- `process_id`: OS process ID
- `thread_id`: OS thread ID
- `api_type`: Hardcoded in SDK, framework wrappers override (Airflow wrapper reports `"airflow"`, not `"python"`)
- `api_version`: TrapperKeeper SDK version
- `os_type`: Operating system (`"linux"`, `"macos"`, `"windows"`, `"freebsd"`, etc. - standardized names)
- `os_version`: OS version
  - macOS/Windows/FreeBSD: Version number (e.g., `"26"` for macOS 26, `"11"` for Windows 11)
  - Linux: Distribution + version (e.g., `"debian-13"`, `"ubuntu-22.04"`)
  - Type: String (not numeric)

### Environment-Specific (Auto-Detected)
- **Kubernetes**: `k8s_pod_name`, `k8s_pod_namespace` (if running in K8s)
  - Detection method: Check env variables first (`KUBERNETES_SERVICE_HOST`), fall back to service account file (`/var/run/secrets/kubernetes.io/serviceaccount/namespace`), else use `TK_META_` override

### User-Provided via Environment Variables
- Environment variables starting with `TK_META_` are auto-collected:
  - `TK_META_FOO_BAR=1234` → metadata key `foo_bar` with value `"1234"`
  - `TK_META_WOMBAT=abcdef` → metadata key `wombat` with value `"abcdef"`
- Always interpreted as strings (UTF-8, no escape characters)
- Lowercase keys, underscores preserved
- Use case: If K8s auto-detection fails, user can explicitly set via `TK_META_K8S_POD_NAME`, etc.

### Collection Timing
- One-time detection: Metadata collected at startup/library import (not per-sensor)
- Cached for process lifetime
- Reduces overhead for short-lived sensors

### Failure Handling
- If auto-detection fails (e.g., can't determine hostname): omit field entirely, log warning
- Non-critical failures don't prevent sensor initialization
- SDKs continue with available metadata
