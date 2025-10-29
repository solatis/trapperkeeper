# ADR-021: Failure Modes and Degradation Strategy
Date: 2025-10-28

## Context

TrapperKeeper sensors operate in distributed environments where network partitions, API outages, and transient failures are inevitable. Key design constraints:

- **Pipeline reliability**: Data processing pipelines must not fail due to observability issues
- **Ephemeral sensors**: Short-lived sensors (minutes to hours) cannot maintain persistent state
- **Network unreliability**: Central API may be unreachable during critical processing windows
- **Stateless protocol**: No persistent connections or heartbeats between sensors and API
- **Initial bootstrap**: Sensors may start in environments where API is temporarily unavailable
- **Configuration diversity**: Different sensors may require different failure handling strategies
- **Least intrusive principle**: System degradation should result in pass-through behavior, not pipeline failures

Traditional monitoring approaches (fail-closed by default, persistent buffering, complex retry logic) conflict with ephemeral sensor architecture and increase operational complexity.

## Decision

We will implement a **fail-safe degradation strategy** where network failures result in transparent pass-through behavior by default, with configurable alternatives for specific use cases.

### 1. Default Fail-Safe Behavior

**When API unreachable**:
- Sensors disable all rules (no observe/drop/error actions)
- Operate as no-op pass-through
- Log warning about offline operation
- Data flows through pipeline unchanged

**Rationale**: Observability system should never break production pipelines. "Least intrusive by default" principle ensures system gracefully degrades to invisible state rather than causing cascading failures.

### 2. Configurable Failure Modes

**Per-sensor configuration** (not per-rule):

```python
sensor = Sensor(
    api_key="...",
    tags=["production"],
    failure_mode="fail-safe"  # Default
)
```

**Available modes**:

| Mode | Behavior | Use Case |
|------|----------|----------|
| `fail-safe` | Disable all rules when offline | Production pipelines (default) |
| `fail-closed` | Raise exception when offline | Critical compliance data where processing must stop |
| `fail-open-with-cache` | Use cached rules with TTL | Environments with intermittent connectivity |

**Configuration scope**:
- Set at sensor initialization time
- Cannot change during sensor lifetime
- Applies to all network-related failures (sync failures, event POST failures, etc.)

**Rationale**: Per-sensor configuration allows flexibility for different pipeline requirements without per-rule complexity. Fail-safe default protects most users while enabling strict mode when needed.

### 3. Initial Rule Sync Failure

**Scenario**: Sensor starts but cannot reach API for initial rule fetch.

**Behavior**:
1. Log warning: "Operating without rules (initial sync failed)"
2. Operate as no-op (no rules = no actions taken)
3. Retry sync after default interval (30 seconds)
4. Continue processing data without blocking

**Rationale**: Aligns with fail-safe principle. Pipeline can start immediately without waiting for API. Rules propagate when connectivity restored.

### 4. Network Partition Handling

**Stateless protocol characteristics**:
- No persistent connections (see ADR-020)
- Periodic rule sync (default: 30 seconds)
- No heartbeat mechanism
- No split-brain detection

**During partition**:
- Sensors operate according to configured failure mode
- No event buffering (events lost during partition)
- Rules cached in-memory until TTL expires
- When cache expires: fall back to failure mode

**After partition resolves**:
- Next sync interval fetches latest rules
- ETAG mechanism prevents unnecessary downloads
- Sensors resume normal operation automatically

**Rationale**: Stateless protocol keeps implementation simple. No event buffering prevents memory exhaustion. Ephemeral sensors don't require split-brain prevention (no persistent identity).

### 5. Rule Caching Strategy

**Cache implementation**:
- In-memory only (no disk cache)
- Lives with sensor object
- Destroyed on sensor exit
- No persistent storage across process restarts

**Cache expiration**:
- Default TTL: 5 minutes after last successful sync
- When cache expires and API unreachable: fall back to configured failure mode
- Cache refreshed on every successful sync

**Rationale**: In-memory cache avoids file locking conflicts with multiple sensors on same host. Simpler implementation with no disk I/O. Sufficient for ephemeral sensors that live minutes to hours.

### 6. Event POST Failures

**Scenario**: Sensor attempts to send events but API returns error or times out.

**Behavior**:
- Log warning with error details
- Discard events (no retry)
- Continue processing data
- Do not fail pipeline

**Rationale**: "Least intrusive by default" principle. Event loss is acceptable compared to pipeline failure. Observability issues should not cause data processing failures.

### 7. Retry and Backoff Strategy

**Rule synchronization**:
- Fixed interval: 30 seconds (configurable)
- No exponential backoff (stateless protocol, short-lived sensors)
- No jitter required (sensors sync independently)

**Event posting**:
- No automatic retry (fail fast, log warning)
- Sensor may explicitly call `flush_events()` to retry
- Future: Configurable retry with exponential backoff

**Rationale**: Simple fixed-interval sync sufficient for ephemeral sensors. Complex retry logic adds little value when sensors live minutes to hours. Operator can adjust sync interval if network conditions require.

### 8. Least Intrusive Principle Applications

**Missing fields**: Default to `skip` (rule doesn't match) rather than error

**Type coercion errors**: Wrap in `on_missing_field` logic, default to skip

**Event POST failures**: Log warning, continue processing

**Network unreachability**: Disable rules (pass-through) rather than fail

**Rule evaluation errors**: Treat as missing field, defer to `on_missing_field` policy

**Rationale**: Consistent application of "least intrusive" principle across all failure scenarios. System degrades gracefully rather than failing noisily.

## Consequences

### Benefits

1. **Pipeline Reliability**: Network issues never break production data processing
2. **Simple Mental Model**: Default behavior is "disappear when broken" (pass-through)
3. **Operational Safety**: Fail-safe default protects against misconfiguration
4. **Flexible Strictness**: Fail-closed mode available for compliance-critical pipelines
5. **No State Management**: In-memory cache eliminates disk locking and cleanup complexity
6. **Graceful Degradation**: System becomes invisible when degraded rather than causing cascading failures
7. **Clear Failure Boundaries**: Event loss acceptable, pipeline failure unacceptable

### Tradeoffs

1. **Event Loss During Partitions**: No buffering means events lost when API unreachable
2. **No Persistent Cache**: Process restart loses cached rules, must re-sync immediately
3. **Silent Failures**: Fail-safe mode may hide connectivity issues (mitigated by logging)
4. **No Automatic Recovery**: Operators must monitor logs for degraded operation warnings
5. **Configuration Inflexibility**: Per-sensor mode cannot change during lifetime
6. **Limited Retry Logic**: Simple fixed-interval sync may not suit all network environments
7. **No Split-Brain Protection**: Partitioned sensors may apply stale rules (acceptable for ephemeral sensors)

### Operational Implications

1. **Monitoring**: Alert on "operating without rules" log warnings to detect API outages
2. **Configuration**: Choose failure mode based on pipeline criticality (prod vs compliance)
3. **Network Design**: Place sensors in same network zone as API for reliability
4. **Graceful Shutdown**: Always flush events before sensor exit to minimize loss
5. **Debugging**: Check sensor logs for offline operation warnings when troubleshooting
6. **API Availability**: High availability critical for fail-closed sensors, less critical for fail-safe

## Implementation

1. Implement failure mode configuration:
   - Add `failure_mode` parameter to sensor initialization
   - Validate mode at startup (reject invalid values)
   - Store mode in sensor object for reference during failures

2. Implement fail-safe behavior:
   - Detect API unreachability during sync
   - Empty rule set when in fail-safe mode and offline
   - Log warning with failure mode and retry interval
   - Continue processing without rules (pass-through)

3. Implement fail-closed behavior:
   - Raise exception when API unreachable in fail-closed mode
   - Provide clear error message with troubleshooting guidance
   - Halt processing until connectivity restored

4. Implement fail-open-with-cache:
   - Track last successful sync timestamp
   - Calculate cache age on each evaluation
   - Use cached rules while TTL valid
   - Fall back to fail-safe when cache expires

5. Implement initial sync failure handling:
   - Attempt sync at sensor initialization
   - Log warning if initial sync fails
   - Set empty rule set (operate as no-op)
   - Schedule retry after default interval

6. Implement event POST failure handling:
   - Catch POST errors in event flush
   - Log warning with error details
   - Discard events (no retry buffer)
   - Return success to caller (continue processing)

7. Implement rule sync retry:
   - Fixed 30-second interval (configurable)
   - No exponential backoff
   - Log each retry attempt with failure reason
   - Update rule set on successful sync

8. Implement cache management:
   - In-memory map of compiled rules
   - Track last sync timestamp
   - Expire cache after TTL (5 minutes default)
   - Clear cache on sensor destruction

## Related Decisions

**Depends on:**
- **ADR-001: Architectural Principles** - Implements the Least Intrusive by Default principle through fail-safe degradation
- **ADR-002: SDK Model** - Defines failure behavior for ephemeral sensors

**Referenced by:**
- **ADR-020: API Service Architecture** - Documents stateless protocol and ETAG-based sync that enables this degradation strategy

## Future Considerations

- **Async event retry**: Background goroutine/thread for exponential backoff retry
- **Dead letter queue**: Persistent buffer for events that fail to send (disk or memory-mapped)
- **Failure metrics**: Expose failure rate and degraded operation time via metrics endpoint
- **Automatic failover**: Multiple API endpoints with automatic failover on failure
- **Circuit breaker**: Temporarily disable API calls after consecutive failures (reduce log noise)
- **Partial degradation**: Continue with subset of cached rules if some rules fail validation
- **Smart caching**: Persist rules to disk for faster recovery after restart
- **Split-brain detection**: Compare rule versions across sensors to detect configuration drift

## Appendix: Failure Mode Decision Tree

```
Sensor attempts operation
  │
  ├─ Initial sync at startup
  │   ├─ Success → Cache rules, operate normally
  │   └─ Failure → Log warning, operate as no-op, retry after 30s
  │
  ├─ Periodic rule sync (every 30s)
  │   ├─ Success → Update cache, reset TTL
  │   └─ Failure (check failure_mode)
  │       ├─ fail-safe → Use cached rules until TTL expires, then empty rule set
  │       ├─ fail-closed → Raise exception (halt processing)
  │       └─ fail-open-with-cache → Use cached rules indefinitely
  │
  └─ Event POST
      ├─ Success → Clear buffer, continue
      └─ Failure → Log warning, discard events, continue
```

## Appendix: Failure Mode Comparison

| Scenario | fail-safe (default) | fail-closed | fail-open-with-cache |
|----------|---------------------|-------------|---------------------|
| API unreachable at startup | Operate as no-op, retry | Exception, halt | Operate as no-op, retry |
| API unreachable during sync | Use cache until TTL, then no-op | Exception, halt | Use cache indefinitely |
| Cache expired, API unreachable | Empty rule set (no-op) | Exception, halt | Use stale cache |
| Event POST failure | Log warning, continue | Log warning, continue | Log warning, continue |
| Network partition (5+ minutes) | Pass-through mode | Pipeline halted | Operate with stale rules |

**Recommended usage**:
- **fail-safe**: Production data pipelines, analytics jobs, non-critical monitoring
- **fail-closed**: Compliance-critical data (GDPR, HIPAA), PII filtering, regulatory reporting
- **fail-open-with-cache**: Edge deployments, intermittent connectivity, mobile sensors
