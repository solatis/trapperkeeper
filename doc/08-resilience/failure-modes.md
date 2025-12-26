---
doc_type: spoke
status: active
date_created: 2025-11-07
primary_category: error-handling
hub_document: doc/08-resilience/README.md
tags:
  - failure-modes
  - degradation
  - network-errors
---

# Failure Modes and Degradation Strategy

## Context

TrapperKeeper sensors operate in distributed environments where network partitions, API outages, and transient failures are inevitable. Ephemeral sensors (short-lived, minutes to hours) cannot maintain persistent state. Fail-safe degradation strategy ensures observability system never breaks production pipelines through transparent pass-through behavior by default, with configurable alternatives for specific use cases.

**Hub Document**: This document is part of the Resilience Architecture hub. See [README.md](README.md) for strategic overview of error handling principles, error taxonomy, and decision trees integrating failure mode selection.

## Default Fail-Safe Behavior

When API unreachable, sensors disable all rules (no observe/drop/error actions) and operate as no-op pass-through. Data flows through pipeline unchanged. Log warning about offline operation.

**Rationale**: Observability system should never break production pipelines. "Least intrusive by default" principle ensures system gracefully degrades to invisible state rather than causing cascading failures.

**Cross-References**:

- Architectural Principles: Least Intrusive principle (Principle 2)

## Configurable Failure Modes

Per-sensor configuration (not per-rule) set at sensor initialization time.

### Available Modes

| Mode                   | Behavior                                                | Use Case                                                                    |
| ---------------------- | ------------------------------------------------------- | --------------------------------------------------------------------------- |
| `fail-safe` (default)  | Disable all rules when offline, operate as pass-through | Production pipelines, analytics jobs, non-critical monitoring               |
| `fail-closed`          | Raise exception when offline, halt processing           | Compliance-critical data (GDPR, HIPAA), PII filtering, regulatory reporting |
| `fail-open-with-cache` | Use cached rules indefinitely when offline              | Edge deployments, intermittent connectivity, mobile sensors                 |

### Configuration Syntax

```python
sensor = Sensor(
    api_key="...",
    tags=["production"],
    failure_mode="fail-safe"  # Default
)
```

**Configuration scope**: Set at sensor initialization time, applies to all network-related failures (sync failures, event POST failures, etc.).

**Cross-References**:

- SDK Model: Ephemeral sensor lifecycle

## Failure Mode Behavior Matrix

| Scenario                       | fail-safe (default)             | fail-closed           | fail-open-with-cache     |
| ------------------------------ | ------------------------------- | --------------------- | ------------------------ |
| API unreachable at startup     | Operate as no-op, retry         | Exception, halt       | Operate as no-op, retry  |
| API unreachable during sync    | Use cache until TTL, then no-op | Exception, halt       | Use cache indefinitely   |
| Cache expired, API unreachable | Empty rule set (no-op)          | Exception, halt       | Use stale cache          |
| Event POST failure             | Log warning, continue           | Log warning, continue | Log warning, continue    |
| Network partition (5+ minutes) | Pass-through mode               | Pipeline halted       | Operate with stale rules |

**Cross-References**:

- Resilience Hub: Decision tree for fail vs degrade vs retry

## Initial Rule Sync Failure

Scenario: Sensor starts but cannot reach API for initial rule fetch.

**Behavior**:

1. Log warning: "Operating without rules (initial sync failed)"
2. Operate as no-op (no rules = no actions taken)
3. Retry sync after default interval (30 seconds)
4. Continue processing data without blocking

**Rationale**: Aligns with fail-safe principle. Pipeline can start immediately without waiting for API. Rules propagate when connectivity restored.

## Network Partition Handling

### Stateless Protocol Characteristics

- No persistent connections
- Periodic rule sync (default: 30 seconds)
- No heartbeat mechanism
- No split-brain detection

### During Partition

- Sensors operate according to configured failure mode
- No event buffering (events lost during partition)
- Rules cached in-memory until TTL expires
- When cache expires: fall back to failure mode

### After Partition Resolves

- Next sync interval fetches latest rules
- ETAG mechanism prevents unnecessary downloads
- Sensors resume normal operation automatically

**Rationale**: Stateless protocol keeps implementation simple. No event buffering prevents memory exhaustion. Ephemeral sensors don't require split-brain prevention (no persistent identity).

**Cross-References**:

- API Service: ETAG-based sync mechanism

## Rule Caching Strategy

### Cache Implementation

- In-memory only (no disk cache)
- Lives with sensor object
- Destroyed on sensor exit
- No persistent storage across process restarts

### Cache Expiration

- Default TTL: 5 minutes after last successful sync
- When cache expires and API unreachable: fall back to configured failure mode
- Cache refreshed on every successful sync

### Ephemeral Sensor Architecture Benefits

In-memory cache suffices for ephemeral sensors because:

1. **Short Sensor Lifespan**: Sensors live minutes to hours—TTL-based expiration works because sensor lifecycle is shorter than typical network partition duration
2. **No Persistent Identity**: Absence of persistent identity means no split-brain scenarios where multiple instances claim the same sensor identity with different cached rule sets
3. **Automatic Cleanup**: Sensors destroyed at job end provide automatic cache cleanup—no stale cache management infrastructure required
4. **Bounded Memory**: Short lifecycle means bounded cache growth—no risk of unbounded accumulation over long-running processes

**Rationale**: In-memory cache avoids file locking conflicts with multiple sensors on same host. Simpler implementation with no disk I/O. Ephemeral architecture makes in-memory cache both sufficient and optimal.

**Cross-References**:

- SDK Model: Ephemeral sensor architecture
- Architectural Principles: Ephemeral Sensors principle (Principle 3)

## Event POST Failures

Scenario: Sensor attempts to send events but API returns error or times out.

**Behavior**:

- Log warning with error details
- Discard events (no retry)
- Continue processing data
- Do not fail pipeline

**Rationale**: "Least intrusive by default" principle. Event loss is acceptable compared to pipeline failure. Observability issues should not cause data processing failures.

## Retry and Backoff Strategy

### Rule Synchronization

- Fixed interval: 30 seconds (configurable)
- No exponential backoff (stateless protocol, short-lived sensors)
- No jitter required (sensors sync independently)

### Event Posting

- No automatic retry (fail fast, log warning)
- Sensor may explicitly call `flush_events()` to retry
- Future: Configurable retry with exponential backoff

**Rationale**: Simple fixed-interval sync sufficient for ephemeral sensors. Complex retry logic adds little value when sensors live minutes to hours. Operator can adjust sync interval if network conditions require.

## Sensor Configuration Patterns

### Pattern 1: Production Pipeline (default)

```python
# Fail-safe mode: Degrade to pass-through when offline
sensor = Sensor(
    api_key="prod_key",
    tags=["analytics", "production"],
    failure_mode="fail-safe"  # Default, can be omitted
)
```

### Pattern 2: Compliance-Critical Processing

```python
# Fail-closed mode: Halt processing when offline
sensor = Sensor(
    api_key="compliance_key",
    tags=["pii", "gdpr"],
    failure_mode="fail-closed"
)
```

### Pattern 3: Edge Deployment with Intermittent Connectivity

```python
# Fail-open-with-cache mode: Use stale rules indefinitely
sensor = Sensor(
    api_key="edge_key",
    tags=["mobile", "offline"],
    failure_mode="fail-open-with-cache"
)
```

**Cross-References**:

- Resilience Hub: Complete decision tree and failure mode selection logic

## Monitoring and Alerting

**Monitoring**: Alert on "operating without rules" log warnings to detect API outages

**Configuration**: Choose failure mode based on pipeline criticality (prod vs compliance)

**Network Design**: Place sensors in same network zone as API for reliability

**Graceful Shutdown**: Always flush events before sensor exit to minimize loss

**Debugging**: Check sensor logs for offline operation warnings when troubleshooting

**API Availability**: High availability critical for fail-closed sensors, less critical for fail-safe

**Cross-References**:

- Resilience Hub: Complete monitoring strategy and alert thresholds
- Operational Endpoints: Health check and metrics exposure

## Edge Cases and Limitations

**Known Limitations**:

- **Event Loss During Partitions**: No buffering means events lost when API unreachable
- **No Persistent Cache**: Process restart loses cached rules, must re-sync immediately
- **Silent Failures**: Fail-safe mode may hide connectivity issues (mitigated by logging)
- **No Automatic Recovery**: Operators must monitor logs for degraded operation warnings
- **Configuration Inflexibility**: Per-sensor mode cannot change during lifetime
- **Limited Retry Logic**: Simple fixed-interval sync may not suit all network environments
- **No Split-Brain Protection**: Partitioned sensors may apply stale rules (acceptable for ephemeral sensors)

**Edge Cases**:

- **Sensor restart during partition**: Starts in fail-safe mode, no rules loaded
- **Intermittent connectivity**: Cache may expire and refresh frequently
- **Concurrent sensor instances**: Each operates independently with own cache
- **Long-running batch job**: May outlive typical sensor lifecycle assumptions

## Related Documents

**Dependencies** (read these first):

- Architectural Principles: Implements Least Intrusive principle through fail-safe degradation
- SDK Model: Defines failure behavior for ephemeral sensors

**Related Spokes** (siblings in this hub):

- Resilience Hub: Complete error handling strategy with decision tree

**Extended by** (documents building on this):

- API Service: Documents stateless protocol and ETAG-based sync that enables this degradation strategy
