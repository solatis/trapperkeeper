---
doc_type: spoke
hub_document: doc/01-principles/README.md
status: active
primary_category: architecture
title: Ephemeral Sensors
tags:
  - sensors
  - lifecycle
  - stateless
  - ephemeral-compute
---

# Ephemeral Sensors

## Context

This document defines the ephemeral sensor model: sensors are short-lived, stateless, and require no server-side registration.

**Hub Document**: See [Architectural Principles](README.md) for the complete set of foundational principles and how they interrelate.

## Core Principle

**Sensors are short-lived by design, tied to job lifecycles.**

Sensors live for minutes to hours, matching the lifecycle of batch jobs,
container tasks, or serverless functions. No registration, no persistent
identity, no disk state.

## Motivation

Modern data pipelines run in ephemeral compute environments:

- **Airflow/Prefect**: Task containers spawn and die with each job run
- **Spark**: Executors scale up/down dynamically
- **Kubernetes**: Pods are cattle, not pets
- **Serverless**: Functions exist only during request handling

Traditional agent-based monitoring assumes long-lived processes with persistent
identity. This creates friction:

- Registration/deregistration lifecycle complexity
- Stale agent cleanup (servers that died without unregistering)
- Persistent state management (disk, databases)
- Health monitoring overhead (heartbeats, timeouts)

Ephemeral design eliminates this complexity by embracing the transient nature of
modern compute.

## Core Characteristics

### Lifecycle Tied to Job

Sensor lifetime matches job/container/function lifetime:

```
Job Start -> Initialize Sensor -> Process Data -> Job End -> Sensor Disappears
```

No explicit lifecycle management:

- No `register()` call
- No `unregister()` call
- No heartbeat/keepalive
- No graceful shutdown protocol (best-effort flush on process exit)

### No Persistent Identity

Sensors have no identity preserved across restarts:

- No sensor ID stored on disk
- No "resume from last checkpoint" logic
- Each sensor instance is independent
- Server has no persistent sensor inventory

**Example:**

```
Airflow Task Run 1: Sensor-A (exists 5 minutes, processes 10K events)
Airflow Task Run 2: Sensor-B (exists 3 minutes, processes 8K events)
```

`Sensor-A` and `Sensor-B` are unrelated. Server doesn't know they're the same
logical task.

### In-Memory State Only

Sensor maintains only in-memory state:

**Rule Cache:**

- Fetched rules cached in RAM
- Cache TTL: 5 minutes (configurable)
- No disk persistence

**Event Buffer:**

- Triggered events buffered for batching
- Bounded in-memory buffer (default: 128 events, 1MB per-event, 128MB total)
- Flushed on buffer full or periodic interval (30s)
- Lost on process crash (no durability guarantee)

**No Disk State:**

- No local database
- No log files (stdout/stderr only)
- No checkpoint files

### Natural Cleanup

Sensor disappears when job completes:

- No explicit cleanup required
- Server-side rule cache entries expire naturally (no sensor ID to invalidate)
- No stale sensor records to purge
- No orphaned state to clean up

### No Health Monitoring

Server doesn't track sensor health:

- No heartbeats from sensors
- No timeout-based failure detection
- No "last seen" timestamps
- No sensor status dashboard

**Rationale:** Health monitoring requires persistent identity and adds
complexity. Sensors are expected to be ephemeral; failure is normal and handled
via fail-safe degradation.

## Benefits

1. **Operational Simplicity**: No sensor lifecycle management, no stale sensor
   cleanup
2. **Alignment with Modern Infra**: Matches container/serverless/batch job
   patterns
3. **Scalability**: No server-side per-sensor state to manage
4. **Simplified SDK**: No registration protocol, no persistent storage layer
5. **Failure Tolerance**: Sensor crashes are non-events (next job run creates
   new sensor)

## Tradeoffs

1. **No Cross-Job Continuity**: Cannot correlate events across job runs
2. **Event Loss on Crash**: In-memory buffer lost if process dies
3. **No Historical Sensor View**: Cannot see "all sensors that ever existed"
4. **Limited Debugging**: Cannot inspect "why did Sensor-X stop reporting"
   (Sensor-X no longer exists)
5. **Cache Inefficiency**: Each new sensor fetches rules (no persistent cache
   across runs)

## Implementation

### SDK Initialization

Minimal initialization (no registration):

```go
sensor := trapperkeeper.NewSensor(SensorConfig{
    ServerURL: "https://trapperkeeper.example.com",
    TeamID: "team-uuid",
    // No sensor ID -- SDK generates ephemeral ID internally if needed
})

// Sensor is ready to use immediately
result := sensor.Evaluate(event)
```

### Rule Fetching

Lazy fetch on first evaluation:

```
evaluate(event) -> rules cached?
  No  -> fetchRules() -> cache in RAM (TTL 5min) -> evaluate
  Yes -> evaluate
```

Cache keyed by team ID, not sensor ID (all sensors in team share rule set).

### Event Buffering

In-memory bounded buffer:

```go
type EventBuffer struct {
    events []Event
    maxSize int       // 1000 events
    maxBytes int64    // 10 MB
    flushInterval time.Duration  // 30 seconds
}
```

Flush triggers:

- Buffer full (size or bytes)
- Flush interval elapsed
- Explicit `sensor.Flush()` call (optional, for graceful shutdown)

No durability: process crash loses buffer contents.

### Server-Side View

Server has no sensor inventory:

- No `sensors` table in database
- No sensor status tracking
- No "active sensors" count
- No sensor health dashboard

Server only knows about:

- Rules (fetched by sensors)
- Events (posted by sensors)
- API keys (authenticate sensor requests)

Sensor is an implementation detail of the SDK, invisible to server.

## Cross-References

- [SDK Model](../02-architecture/sdk-model.md) - SDK library integration model
  implements ephemeral lifecycle
- [API Service Architecture](../02-architecture/api-service.md) - Stateless
  gRPC protocol supports ephemeral sensors
- [Failure Modes](../08-resilience/failure-modes.md) - Fail-safe mode handles
  sensor crashes gracefully
- [Event Schema and Storage](../03-data/event-schema-storage.md) - Event
  buffering and flush strategies

## Future Considerations

### Optional Persistent Identity

For customers needing cross-job correlation:

- Opt-in sensor ID persistence (environment variable or config file)
- SDK reads/writes sensor ID to disk
- Server tracks sensor "sessions" for correlation
- Backward compatible: default behavior remains ephemeral

Example use case: Correlate events from daily Airflow runs of same task.

### Event Durability

For critical audit scenarios:

- Opt-in local event log (append-only file on disk)
- SDK writes events to local log before buffering
- Separate background process ships log to server
- Survives process crashes

Tradeoff: Adds disk I/O overhead, requires disk space management.

### Sensor Telemetry

For operational visibility without persistent identity:

- SDK exposes Prometheus metrics endpoint (in-process HTTP server)
- Metrics include: rules cached, events buffered, flush rate
- External scraper (Prometheus, Datadog) polls metrics
- No server-side state required

Enables monitoring ephemeral sensors without centralized tracking.
