---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: architecture
hub_document: /Users/lmergen/git/trapperkeeper/doc/02-architecture/README.md
tags:
  - sdk
  - ephemeral
  - buffer
  - fail-safe
---

# SDK Model

## Context

TrapperKeeper SDKs must integrate seamlessly with data processing frameworks (Airflow DAGs, Spark jobs, Kubernetes pods) where workloads are ephemeral and run for minutes to hours. Traditional SDK patterns assuming long-lived services and persistent connections do not fit this environment.

The SDK model prioritizes developer experience through minimal boilerplate, explicit control over buffers and network I/O, and fail-safe defaults that degrade gracefully when dependencies are unavailable. SDKs are pure native implementations using gRPC client libraries, avoiding server-side dependencies to keep distributions lean (~5-10MB).

**Hub Document**: This document is part of the Architecture Hub. See Architecture Overview for strategic context on developer-first SDK design within TrapperKeeper's two-service architecture.

## Ephemeral Sensor Pattern

Sensors live for the duration of the workload, matching ephemeral compute patterns.

### Sensor Lifecycle

```python
# Sensor created at job start
sensor = Sensor(api_key=os.environ['TK_API_KEY'])

# Sensor used during job execution (minutes to hours)
for record in data_source:
    sensor.observe(record)

# Sensor destroyed at job end (automatic or explicit)
sensor.flush()
# Sensor garbage collected, no cleanup required
```

**Key Characteristics**:

- No persistent registration: Sensor authenticates per request using API key
- No long-lived connections: gRPC connection established on demand
- No cleanup required: Process termination handles all cleanup
- Stateless protocol: Server maintains no session state for sensors

**Benefits**:

- Aligns with Airflow DAG task execution model
- Simplifies Kubernetes pod lifecycle management
- Reduces operational burden (no sensor registry to maintain)
- Enables horizontal scaling without coordination

**Cross-References**:

- API Service Architecture Section 5: Stateless protocol design
- Principles Architecture Principle #3: Ephemeral sensors
- Failure Modes and Degradation: Fail-safe defaults for ephemeral sensors

### Example: Airflow DAG Task

```python
@task
def process_batch(batch_id: str):
    # Sensor created for this task only
    sensor = Sensor(
        api_key=os.environ['TK_API_KEY'],
        metadata={'airflow_dag_id': dag_id, 'airflow_task_id': task_id}
    )

    # Process data
    for record in fetch_batch(batch_id):
        sensor.observe(record)

    # Explicit flush before task completes
    sensor.flush()
    # Task ends, sensor destroyed
```

## Pre-Compiled Rule Distribution

Rules are compiled on the server and synchronized to sensors as pre-compiled bytecode.

### Compilation Flow

1. **Rule Creation**: Operator defines rule expression via Web UI (e.g., `$.temperature > 80`)
2. **Server Compilation**: Server parses expression into DNF representation and validates
3. **Rule Storage**: Compiled rule stored in database with UUIDv7 identifier
4. **Rule Sync**: Sensor requests rules via `SyncRules` RPC, receives pre-compiled bytecode
5. **SDK Execution**: SDK evaluates events against pre-compiled rules (zero parsing overhead)

**Benefits**:

- Zero parsing overhead: SDKs evaluate pre-compiled rules directly
- Consistent semantics: All sensors evaluate rules identically
- Simplified SDK implementation: No expression parser required
- Centralized validation: Invalid expressions rejected at creation time

**Rule Sync Protocol**:

```python
sensor = Sensor(api_key=api_key)

# First sync: Fetch all rules
rules = sensor.sync_rules()  # Returns list of pre-compiled Rule objects

# Subsequent syncs: ETAG-based conditional requests
rules = sensor.sync_rules()  # Returns empty list if no changes (ETAG match)
```

**ETAG Caching**: SDK caches ETAG from sync response, includes in subsequent requests. Server returns empty response if ETAG matches (no changes).

**Cross-References**:

- API Service Architecture Section 1: SyncRules RPC specification
- Rule Expression Language: DNF evaluation semantics
- Rule Lifecycle: Rule compilation and state transitions

## Explicit Buffer Management

Developers control when events are sent to the server via explicit `flush()` calls.

### Buffer Pattern

```python
sensor = Sensor(api_key=api_key, buffer_size=1000)

# Events buffered locally
for i in range(5000):
    sensor.observe({"value": i})
    # No network I/O yet

# Explicit flush sends all buffered events
results = sensor.flush()
print(f"Accepted: {results.accepted_count}, Rejected: {results.rejected_count}")
```

**Buffer Behavior**:

- Events accumulated in memory until `flush()` called or buffer limit reached
- Automatic synchronous flush when buffer reaches 128 events (configurable)
- Auto-flush executes inline with `observe()` -- no background threads
- No hidden network I/O except when buffer limit triggers auto-flush
- Developer controls timing and error handling via explicit `flush()`
- Buffer size configurable (default: 128 events, with 1MB per-event and 128MB total limits)

**Benefits**:

- Predictable performance: No surprise network calls
- Explicit error boundaries: Failures scoped to flush operation
- Memory control: Developers manage buffer size and flush frequency
- Testing friendly: Easy to unit test without mocking network

**Auto-Flush Pattern**:

```python
sensor = Sensor(api_key=api_key, buffer_size=128)

# Auto-flush happens automatically when buffer reaches limit
for record in large_dataset:
    sensor.observe(record)
    # Automatic synchronous flush when 128 events buffered
    # No background threads -- flush executes inline with observe()

# Final flush for remaining events (less than buffer_size)
sensor.flush()
```

**Auto-Flush Behavior**:

- SDK automatically flushes when buffer reaches configured limit (default: 128 events)
- Flush is synchronous and inline with `observe()` call, not a background thread
- Enables processing arbitrarily large batches without manual chunking
- For large datasets, processing continues seamlessly across multiple auto-flush cycles
- Example: 1000 events with 128-event buffer triggers ~8 automatic flushes

**Cross-References**:

- API Service Architecture Section 1: ReportEvents RPC batch processing
- Batch Processing and Vectorization: Performance optimizations for large datasets
- Testing Examples Section 2: Buffer management testing

## Fail-Safe Defaults

SDKs implement fail-safe degradation strategies when dependencies are unavailable.

### Failure Mode Configuration

```python
sensor = Sensor(
    api_key=api_key,
    on_api_failure='fail_safe',  # Options: fail_safe, fail_closed, fail_open
    cache_rules=True,             # Cache rules for offline evaluation
    cache_ttl=3600               # Cache TTL: 1 hour
)
```

**Failure Modes**:

- **fail_safe** (default): Use cached rules, log warnings, continue processing
- **fail_closed**: Treat all events as violations when API unavailable
- **fail_open**: Skip rule evaluation when API unavailable, assume pass

**Rule Caching**:

- SDK caches rules from last successful sync
- Cache persists across process restarts (optional)
- Cache TTL configurable (default: 1 hour)
- Expired cache triggers sync attempt, falls back to cached rules if sync fails

**Benefits**:

- Resilient to transient failures: Short network blips don't halt processing
- Graceful degradation: Processing continues with potentially stale rules
- Least intrusive: Fail-safe default minimizes pipeline disruption
- Observable: All degraded operations logged for monitoring

### Example: API Unreachable

```python
sensor = Sensor(api_key=api_key, on_api_failure='fail_safe')

# First sync succeeds, rules cached
sensor.sync_rules()  # → 10 rules, ETAG cached

# API becomes unreachable
# Sensor continues using cached rules, logs warnings
for record in data_source:
    result = sensor.observe(record)  # Uses cached rules
    # Warning logged: "API unreachable, using cached rules (age: 5 minutes)"

# API recovers
sensor.sync_rules()  # Fetches updated rules, refreshes cache
```

**Cross-References**:

- Failure Modes and Degradation: Complete failure mode decision tree
- Principles Architecture Principle #2: Least intrusive by default
- Error Handling Strategy: Network error handling patterns

## Language-Specific SDK Bindings

Each SDK is a pure native implementation using the language's gRPC client library. All SDKs share a common conformance test suite to ensure consistent rule evaluation semantics across languages.

### Python SDK (grpc-python)

Package: `sdks/python/trapperkeeper/`

```python
# Pure Python implementation using grpc-python
from trapperkeeper import Sensor

sensor = Sensor(api_key=api_key)
sensor.observe({"temperature": 95.5})
sensor.flush()
```

**Python-Specific Features**:

- Context manager support: `with Sensor(...) as sensor:`
- Pandas DataFrame integration (see Batch Processing and Vectorization)
- Type hints for IDE autocompletion
- Pythonic error handling with standard exceptions

**Build**: Standard Python package with grpc dependencies

**Distribution**: PyPI package (~5-10MB)

**Cross-References**:

- Batch Processing and Vectorization: Pandas integration patterns
- Monorepo Directory Structure: Python SDK organization

### Java SDK (grpc-java)

Package: `sdks/java/trapperkeeper/`

```java
// Pure Java implementation using grpc-java
import ai.trapperkeeper.Sensor;

Sensor sensor = new Sensor(apiKey);
sensor.observe(Map.of("temperature", 95.5));
sensor.flush();
```

**Java-Specific Features**:

- Try-with-resources support: `try (Sensor sensor = new Sensor(...))`
- Spark DataFrame integration (see Batch Processing and Vectorization)
- Java Optional for nullable values
- Standard Java exceptions

**Build**: Gradle builds JAR with gRPC dependencies

**Distribution**: Maven Central artifact (~8-12MB JAR)

**Cross-References**:

- Batch Processing and Vectorization: Spark integration patterns
- Monorepo Directory Structure: Java SDK organization

### Go SDK (grpc-go)

Package: `sdks/go/trapperkeeper/`

```go
// Pure Go implementation using grpc-go
import "github.com/trapperkeeper/sdk-go"

sensor := trapperkeeper.NewSensor(apiKey)
sensor.Observe(map[string]interface{}{"temperature": 95.5})
sensor.Flush()
```

**Go-Specific Features**:

- Defer pattern support: `defer sensor.Flush()`
- Context-aware operations for cancellation and timeouts
- Idiomatic error handling with returned errors
- Type-safe structured data using interfaces

**Build**: Standard Go module

**Distribution**: Go module (~5-8MB binary)

**Cross-References**:

- Monorepo Directory Structure: Go SDK organization

### Conformance Testing

All SDK implementations must pass a shared conformance test suite ensuring:

- **Rule Evaluation Consistency**: Identical rule evaluation results across languages
- **Type Coercion Semantics**: Consistent type conversion behavior
- **Field Path Resolution**: Identical wildcard expansion and nested path handling
- **DNF Evaluation**: Same short-circuit and evaluation order guarantees

**Test Suite Location**: `tests/conformance/`

**Cross-References**:

- Testing Philosophy: Conformance testing strategy
- Type System and Coercion: Language-specific type mapping requirements

## SDK Testing Boundaries

Clear boundaries define what each SDK tests to avoid duplication.

### Go SDK (Reference Implementation)

Tests `sensor-api` service thoroughly as reference:

- Validates partial batch failures (some records succeed, some fail)
- Tests dead-letter queue (DLQ) behavior when events fail submission
- Validates rule syncing with ETAG caching
- Tests authentication failures and retry logic
- Validates fail-safe mode when API unreachable

### Language-Specific SDKs

Each SDK tests language-specific concerns only:

**Python SDK Tests**:

- Python-specific data type handling (datetime, Decimal, numpy arrays)
- Pandas DataFrame integration correctness
- UTF-8 encoding from Python's internal UTF-32 representation
- End-to-end: Data submitted via Python SDK appears correctly in database

**Java SDK Tests**:

- Java-specific type conversions (LocalDateTime, BigDecimal)
- Spark DataFrame integration correctness
- UTF-8 encoding from Java String representation
- End-to-end: Data submitted via Java SDK appears correctly in database

**Go SDK Tests**:

- Go-specific type conversions (time.Time, big.Float)
- Context handling and cancellation semantics
- UTF-8 encoding from Go string representation
- End-to-end: Data submitted via Go SDK appears correctly in database

**Rationale**: Go SDK validates server behavior comprehensively. Language SDKs validate language bindings and type conversions only. Avoids testing server logic N times.

**Cross-References**:

- Testing Philosophy Section 4: SDK testing boundaries
- Testing Examples Section 2: SDK integration test examples

## SDK Metadata Collection

SDKs automatically collect system metadata for event correlation.

### Automatic Metadata

```python
sensor = Sensor(api_key=api_key)  # Auto-collects metadata

# Automatically added to all events:
# $tk.api_type = "python"
# $tk.api_version = "0.1.0"
# $tk.client_ip = "192.168.1.100"
# $tk.client_timestamp = "2025-10-29T10:00:00Z"
```

### Environment Variable Scanning

```bash
# Environment variables with TK_META_ prefix automatically collected
export TK_META_AIRFLOW_DAG_ID=daily_etl_pipeline
export TK_META_AIRFLOW_TASK_ID=process_batch
export TK_META_K8S_POD_NAME=etl-worker-abc123

# Python SDK automatically includes:
# airflow_dag_id = "daily_etl_pipeline"
# airflow_task_id = "process_batch"
# k8s_pod_name = "etl-worker-abc123"
```

### Custom Metadata

```python
sensor = Sensor(
    api_key=api_key,
    metadata={
        'team': 'data-platform',
        'environment': 'production',
        'job_id': job_id,
    }
)

# All events include custom metadata for correlation
```

**Metadata Limits**:

- Max 64 key-value pairs per sensor
- Max 128 characters per key (UTF-8)
- Max 1024 characters per value (UTF-8)
- Max 64KB total metadata size
- Keys cannot start with `$` (reserved for system metadata)

**Cross-References**:

- Client Metadata Namespace: Complete metadata specification
- Event Schema and Storage: Metadata validation rules

## SDK API Surface

Minimal API surface reduces learning curve and maintenance burden.

### Core API

```python
# Initialization
sensor = Sensor(api_key: str, **options)

# Rule synchronization
rules: List[Rule] = sensor.sync_rules()

# Event submission
sensor.observe(data: dict)
sensor.observe_many(data: List[dict])

# Explicit flush
results: FlushResults = sensor.flush()

# Buffer status
count: int = sensor.buffered_count()

# Diagnostics
info: DiagnosticInfo = sensor.diagnostics()
```

**Optional Helpers**:

```python
# Context manager (Python)
with Sensor(api_key) as sensor:
    sensor.observe(data)
    # Auto-flushes on __exit__

# Dry-run mode (testing)
sensor = Sensor(api_key, dry_run=True)  # No network I/O

# Pandas integration
import pandas as pd
sensor.observe_dataframe(df, record_column='data')
```

**Cross-References**:

- API Service Architecture: gRPC methods backing SDK API
- Testing Philosophy: Dry-run mode for testing

## Related Documents

**Dependencies** (read these first):

- Architecture Overview: Developer-first SDK design philosophy
- Principles Architecture: Ephemeral sensors and least intrusive principles

**Related Spokes** (siblings in this hub):

- API Service Architecture: gRPC protocol SDKs communicate with
- Binary Distribution Strategy: SDK packaging and distribution

**Extended by**:

- Failure Modes and Degradation: Complete fail-safe strategy
- Batch Processing and Vectorization: Pandas and Spark integration patterns
- Client Metadata Namespace: Metadata specification and limits
- Monorepo Directory Structure: Module organization enabling lean SDK distributions
